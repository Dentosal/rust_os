use alloc::vec::Vec;
use hashbrown::HashSet;

use super::*;

#[derive(Debug)]
pub struct SubscriptionList {
    /// Tuples of (filter, reliable, subscription_id)
    /// Must be kept sorted, used for binary search queries
    targets: Vec<(TopicFilter, bool, SubscriptionId)>,
    /// Next free subscription id
    next_subscription_id: SubscriptionId,
}
impl SubscriptionList {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            next_subscription_id: SubscriptionId::from_u64(0),
        }
    }

    /// Returns None on exclusion conflict
    pub fn insert(&mut self, filter: TopicFilter, reliable: bool) -> Option<SubscriptionId> {
        if self.conflicts(&filter, reliable) {
            return None;
        }

        let id = self.next_subscription_id;
        let value = (filter, reliable, id);
        let index = match self.targets.binary_search(&value) {
            Ok(i) => i,
            Err(i) => i,
        };
        self.targets.insert(index, value);
        self.next_subscription_id = self.next_subscription_id.next();
        Some(id)
    }

    /// Checks whether any topic matching a filter exists,
    /// used for exlusion checks.
    fn conflicts(&self, filter: &TopicFilter, reliable: bool) -> bool {
        // TODO: Use binary search for more speed
        for (f, r, _) in &self.targets {
            if (reliable && filter.contains_filter(f)) || (*r && f.contains_filter(filter)) {
                return true;
            }
        }
        return false;
    }

    pub fn remove(&mut self, subscription: SubscriptionId) {
        self.targets.retain(|(_, _, id)| subscription != *id);
    }

    pub fn find_all(&self, topic: &Topic, reliable: bool) -> HashSet<SubscriptionId> {
        // TODO: Use binary search for more speed

        self.targets
            .iter()
            .filter_map(|(f, r, id)| {
                if *r == reliable && f.matches(topic) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! set {
        ($($x:expr),*) => ({
            let mut hs = HashSet::new();
            $(
                hs.insert($x);
            )*
            hs
        });
        ($($x:expr,)*) => (set![$($x),*])
    }

    #[test]
    #[rustfmt::skip]
    fn test_insert_exclusion_0() {
        let topic_a = Topic::new("a").unwrap();
        let topic_b = Topic::new("b").unwrap();
        let topic_c = Topic::new("c").unwrap();

        let mut list = SubscriptionList::new();

        list.insert(TopicFilter::Exact(topic_a.clone()), true).unwrap();
        list.insert(TopicFilter::Exact(topic_b.clone()), true).unwrap();
        list.insert(TopicFilter::Exact(topic_c.clone()), false).unwrap();
        list.insert(TopicFilter::Exact(topic_c.clone()), false).unwrap();
        assert!(list.insert(TopicFilter::Exact(topic_a.clone()), false).is_none());
        assert!(list.insert(TopicFilter::Exact(topic_a.clone()), true).is_none());
        assert!(list.insert(TopicFilter::Exact(topic_c.clone()), true).is_none());
    }

    #[test]
    #[rustfmt::skip]
    fn test_insert_exclusion_1() {
        let topic_prefix = TopicPrefix::new("a/").unwrap();
        let topic_a0 = Topic::new("a").unwrap();
        let topic_a1 = Topic::new("a/a").unwrap();
        let topic_b0 = Topic::new("b").unwrap();
        let topic_b1 = Topic::new("b/a").unwrap();

        let mut list = SubscriptionList::new();
        list.insert(TopicFilter::Prefix(topic_prefix.clone()), true).unwrap();

        assert!(list.insert(TopicFilter::Exact(topic_a0), false).is_some());
        assert!(list.insert(TopicFilter::Exact(topic_a1), false).is_none());
        assert!(list.insert(TopicFilter::Exact(topic_b0), false).is_some());
        assert!(list.insert(TopicFilter::Exact(topic_b1), false).is_some());
    }

    #[test]
    #[rustfmt::skip]
    fn test_insert_exclusion_2() {
        let topic_prefix = TopicPrefix::new("a/").unwrap();
        let topic_a0 = Topic::new("a").unwrap();
        let topic_a1 = Topic::new("a/a").unwrap();
        let topic_b0 = Topic::new("b").unwrap();
        let topic_b1 = Topic::new("b/a").unwrap();

        let mut list = SubscriptionList::new();
        list.insert(TopicFilter::Prefix(topic_prefix.clone()), false).unwrap();

        assert!(list.insert(TopicFilter::Exact(topic_a0), true).is_some());
        assert!(list.insert(TopicFilter::Exact(topic_a1), true).is_none());
        assert!(list.insert(TopicFilter::Exact(topic_b0), true).is_some());
        assert!(list.insert(TopicFilter::Exact(topic_b1), true).is_some());
    }

    #[test]
    #[rustfmt::skip]
    fn test_insert_exclusion_3() {
        let topic_a0 = Topic::new("a").unwrap();
        let topic_a1 = Topic::new("a/a").unwrap();
        let topic_p0 = TopicPrefix::new("a").unwrap();
        let topic_p1 = TopicPrefix::new("a/").unwrap();
        let topic_p2 = TopicPrefix::new("a/a").unwrap();

        let mut list = SubscriptionList::new();
        list.insert(TopicFilter::Exact(topic_a0.clone()), true).unwrap();
        assert!(list.insert(TopicFilter::Exact(topic_a0.clone()), false).is_none());
        assert!(list.insert(TopicFilter::Exact(topic_a1.clone()), false).is_some());
        assert!(list.insert(TopicFilter::Prefix(topic_p0.clone()), false).is_none());
        assert!(list.insert(TopicFilter::Prefix(topic_p1.clone()), false).is_some());
        assert!(list.insert(TopicFilter::Prefix(topic_p2.clone()), false).is_some());

        let mut list = SubscriptionList::new();
        list.insert(TopicFilter::Exact(topic_a1.clone()), true).unwrap();
        assert!(list.insert(TopicFilter::Exact(topic_a0), false).is_some());
        assert!(list.insert(TopicFilter::Exact(topic_a1), false).is_none());
        assert!(list.insert(TopicFilter::Prefix(topic_p0), false).is_none());
        assert!(list.insert(TopicFilter::Prefix(topic_p1), false).is_none());
        assert!(list.insert(TopicFilter::Prefix(topic_p2), false).is_none());
    }

    #[test]
    #[rustfmt::skip]
    fn test_find_all() {
        let f0 = TopicFilter::Exact(Topic::new("a").unwrap());
        let f1 = TopicFilter::Exact(Topic::new("a/b").unwrap());
        let f2 = TopicFilter::Exact(Topic::new("a/b/c").unwrap());
        let f3 = TopicFilter::Prefix(TopicPrefix::new("a").unwrap());
        let f4 = TopicFilter::Prefix(TopicPrefix::new("a/").unwrap());
        let f5 = TopicFilter::Prefix(TopicPrefix::new("a/b").unwrap());
        let f6 = TopicFilter::Prefix(TopicPrefix::new("a/b/").unwrap());

        let mut list = SubscriptionList::new();
        let id_f0 = list.insert(f0, false).unwrap();
        let id_f1 = list.insert(f1, false).unwrap();
        let id_f2 = list.insert(f2, false).unwrap();
        let id_f3 = list.insert(f3, false).unwrap();
        let id_f4 = list.insert(f4, false).unwrap();
        let id_f5 = list.insert(f5, false).unwrap();
        let id_f6 = list.insert(f6, false).unwrap();

        let topic0 = Topic::new("a").unwrap();
        let topic1 = Topic::new("a/b").unwrap();
        let topic2 = Topic::new("a/b/c").unwrap();
        let topic3 = Topic::new("nonexistent").unwrap();

        let l0 = list.find_all(&topic0, false);
        let l1 = list.find_all(&topic1, false);
        let l2 = list.find_all(&topic2, false);
        let l3 = list.find_all(&topic3, false);

        assert_eq!(l0, set![id_f0, id_f3]);
        assert_eq!(l1, set![id_f1, id_f3, id_f4, id_f5]);
        assert_eq!(l2, set![id_f2, id_f3, id_f4, id_f5, id_f6]);
        assert_eq!(l3, set![]);

        list.find_all(&topic0, true).is_empty();
        list.find_all(&topic1, true).is_empty();
        list.find_all(&topic2, true).is_empty();
        list.find_all(&topic3, true).is_empty();
    }
}
