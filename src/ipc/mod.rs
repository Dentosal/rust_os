//! Asynchronous IPC message queues using variable-guarantee pub-sub model
//!
//! Example use cases:
//! * New processable interrupt: Reliable, ignore no-targets case
//! * System shutdown requested: Unreliable
//!
//! TODO: multi-reader reliable delivery?
//! TODO: permission system
//! TODO: page mapping for large messages

use alloc::string::String;
use hashbrown::{HashMap, HashSet};
use spin::Mutex;

use d7abi::process::ProcessResult;

pub use d7abi::ipc::{AcknowledgeId, Message, SubscriptionId};

use crate::multitasking::{ExplicitEventId, Process, ProcessId, Scheduler, WaitFor};

mod event_queue;
mod list;
mod result;
mod topic;

use self::event_queue::EventQueue;
use self::list::SubscriptionList;

pub use self::result::*;
pub use self::topic::{Topic, TopicFilter, TopicPrefix};

/// A mailbox will reject (reliable) or drop (unreliable)
/// messages after it's buffer contains this many messages.
const MAILBOX_BUFFER_LIMIT: usize = 100;

#[derive(Debug)]
enum PipeMode {
    /// This is not a pipe mailbox
    None,
    /// No process has connected yet
    NotConnected,
    /// Process has connected
    ConnectedTo(ProcessId),
    /// Previously-connected process has been terminated
    Disconnected,
}

#[derive(Debug)]
struct Mailbox {
    queue: EventQueue<Message>,
    pub pipe_mode: PipeMode,
}
impl Mailbox {
    pub fn new(pipe_mode: PipeMode) -> Self {
        Self {
            queue: EventQueue::new(MAILBOX_BUFFER_LIMIT),
            pipe_mode,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[must_use]
    pub fn push_unreliable(&mut self, message: Message) -> Option<TriggerEvent> {
        assert!(matches!(self.pipe_mode, PipeMode::None));
        match self.queue.push(message) {
            Ok(v) => v.map(TriggerEvent),
            Err(()) => {
                log::debug!("Unreliable delivery queue full");
                None
            },
        }
    }

    /// Caller must ensure that PipeMode is updated and checked first
    #[must_use]
    pub fn push_reliable(
        &mut self, message: Message,
    ) -> Result<Option<TriggerEvent>, DeliveryError> {
        assert!(!matches!(self.pipe_mode, PipeMode::NotConnected));
        self.queue
            .push(message)
            .map(|opt| opt.map(TriggerEvent))
            .map_err(|()| DeliveryError::QueueFull)
    }

    #[must_use]
    pub fn pop_or_event(&mut self) -> Result<Message, ExplicitEventId> {
        self.queue.pop_or_event()
    }
}

/// Result of Manager::deliver
#[derive(Debug)]
pub enum Deliver {
    /// Wait for this event
    Process(ExplicitEventId),
    /// Just wake up with ok
    Kernel,
}

macro_rules! verify_owner {
    ($self_:ident, $pid:ident, $sub:ident) => {
        if let Err(e) = $self_.verify_process_owns($pid, $sub) {
            return IpcResult::error(e.into());
        }
    };
}

#[derive(Debug)]
pub struct Manager {
    /// Topic to subscription id mapping
    subscriptions: SubscriptionList,
    /// All mailboxes by subscription id.
    /// Mailbox is None if the message is handled byu the kernel instead.
    mailboxes: HashMap<SubscriptionId, Option<Mailbox>>,
    /// Reliable messages waiting for the receiver acknowledgement.
    /// The value field contains are sender wakeup id and process id.
    waiting_for_delivery: HashMap<AcknowledgeId, (ExplicitEventId, ProcessId)>,
    /// Reliable messages that have been delivered (or caused an error).
    /// The value field contains success status.
    delivery_result: HashMap<ProcessId, Result<(), DeliveryError>>,
    /// Next free acknowledge id
    next_acknowledge_id: AcknowledgeId,
    /// ProcessId -> SubscriptionId mapping for process-exit cleanup
    process_subscriptions: HashMap<ProcessId, HashSet<SubscriptionId>>,
}
impl Manager {
    pub fn new() -> Self {
        Self {
            subscriptions: SubscriptionList::new(),
            mailboxes: HashMap::new(),
            waiting_for_delivery: HashMap::new(),
            delivery_result: HashMap::new(),
            next_acknowledge_id: AcknowledgeId::from_u64(0),
            process_subscriptions: HashMap::new(),
        }
    }

    /// Return if a subscription is owned by a process
    fn process_owns(&self, pid: ProcessId, sub: SubscriptionId) -> bool {
        self.process_subscriptions
            .get(&pid)
            .map(|subs| subs.contains(&sub))
            .unwrap_or(false)
    }

    /// Return an error if process doesn't own a subscription
    #[must_use]
    fn verify_process_owns(
        &self, pid: ProcessId, sub: SubscriptionId,
    ) -> Result<(), PermissionError> {
        if self.process_owns(pid, sub) {
            Ok(())
        } else {
            Err(PermissionError::NotOwner)
        }
    }

    /// Subscribe to normal events by a filter
    /// Reliable subscriptions are mutually exclusive: there cannot be
    /// any other endpoint subscribed to the any events matched by this.
    pub fn subscribe(
        &mut self, pid: ProcessId, filter: TopicFilter, reliable: bool, pipe: bool,
    ) -> Result<SubscriptionId, SubscriptionError> {
        if let Some(id) = self.subscriptions.insert(filter, reliable) {
            self.mailboxes.insert(
                id,
                Some(Mailbox::new(if pipe {
                    PipeMode::NotConnected
                } else {
                    PipeMode::None
                })),
            );
            self.process_subscriptions
                .entry(pid)
                .or_default()
                .insert(id);
            Ok(id)
        } else {
            Err(SubscriptionError::Exclusion.into())
        }
    }

    /// Subscribe to events by a filter as a kernel.
    /// Only supports reliable subscriptions.
    pub fn kernel_subscribe(
        &mut self, filter: TopicFilter,
    ) -> Result<SubscriptionId, SubscriptionError> {
        if let Some(id) = self.subscriptions.insert(filter, true) {
            self.mailboxes.insert(id, None);
            Ok(id)
        } else {
            Err(SubscriptionError::Exclusion.into())
        }
    }

    /// Internal function for removing subscriptions on process termination
    pub fn _force_unsubscribe(&mut self, subscription: SubscriptionId) -> IpcResult<()> {
        self.subscriptions.remove(subscription);
        let mailbox = self
            .mailboxes
            .remove(&subscription)
            .unwrap()
            .expect("Kernel cannot unsubscribe");

        // Release reliable messages
        let mut events = HashSet::new();
        for msg in mailbox.queue.into_iter() {
            if let Some(ack_id) = msg.ack_id {
                let (event, pid) = self.waiting_for_delivery.remove(&ack_id).unwrap();
                self.delivery_result
                    .insert(pid, Err(DeliveryError::NoSubscriber));
                events.insert(TriggerEvent(event));
            }
        }
        IpcResult::success(()).with_events(events.into_iter())
    }

    /// Remove a subscription.
    pub fn unsubscribe(&mut self, pid: ProcessId, subscription: SubscriptionId) -> IpcResult<()> {
        verify_owner!(self, pid, subscription);
        self.process_subscriptions
            .get_mut(&pid)
            .unwrap()
            .remove(&subscription);
        self._force_unsubscribe(subscription)
    }

    /// Unreliable (fire-and-forget) publish to a key group
    pub fn publish(&mut self, topic: Topic, data: &[u8]) -> IpcResult<()> {
        let mut events = HashSet::new();
        for sub in self.subscriptions.find_all(&topic, false) {
            let mailbox = self
                .mailboxes
                .get_mut(&sub)
                .unwrap()
                .as_mut()
                .expect("Cannot send unreliable messages to the kernel");
            events.extend(
                mailbox
                    .push_unreliable(Message {
                        topic: topic.string(),
                        data: data.to_vec(),
                        ack_id: None,
                    })
                    .iter(),
            )
        }
        IpcResult::success(()).with_events(events.into_iter())
    }

    /// Reliable delivery to exclusive topic.
    /// The caller must repeat the call after the returned event has been
    /// triggered by the receiving process, i.e. `WaitFor::Event`
    /// (or `WaitFor::None` if kernel processes the message immediately).
    pub fn deliver(&mut self, pid: ProcessId, topic: Topic, data: &[u8]) -> IpcResult<Deliver> {
        let all = self.subscriptions.find_all(&topic, true);
        let count = all.len();
        if all.len() == 0 {
            log::warn!("Delivery error: no subscribers for {:?}", topic);
            return IpcResult::error(DeliveryError::NoSubscriber.into());
        }
        assert!(
            count == 1,
            "Multiple targets selected for reliable delivery"
        );
        let ack_id = self.next_acknowledge_id;
        self.next_acknowledge_id = self.next_acknowledge_id.next();
        let sub = all.into_iter().next().unwrap();
        if let Some(mailbox) = self.mailboxes.get_mut(&sub).unwrap() {
            // Deliver to another process

            match mailbox.pipe_mode {
                PipeMode::None => {},
                PipeMode::NotConnected => {
                    mailbox.pipe_mode = PipeMode::ConnectedTo(pid);
                },
                PipeMode::ConnectedTo(c_pid) => {
                    if c_pid != pid {
                        return Error::PipeReserved.into();
                    }
                },
                PipeMode::Disconnected => {
                    return Error::PipeReserved.into();
                },
            }

            let result = mailbox.push_reliable(Message {
                topic: topic.string(),
                data: data.to_vec(),
                ack_id: Some(ack_id),
            });

            match result {
                Ok(trigger) => {
                    let sender_wakeup_id = WaitFor::new_event_id();
                    self.waiting_for_delivery
                        .insert(ack_id, (sender_wakeup_id, pid));
                    IpcResult::success(Deliver::Process(sender_wakeup_id))
                        .with_events(trigger.into_iter())
                },
                Err(error) => IpcResult::error(error.into()),
            }
        } else {
            // Deliver to kernel
            crate::services::incoming(self, pid, sub, Message {
                topic: topic.string(),
                data: data.to_vec(),
                ack_id: Some(ack_id),
            })
            .map(|()| Deliver::Kernel)
        }
    }

    /// Reply to a delivery to a different topic.
    /// The other party must be blocked by deliver for this to be used.
    pub fn deliver_reply(&mut self, pid: ProcessId, topic: Topic, data: &[u8]) -> IpcResult<()> {
        let all = self.subscriptions.find_all(&topic, true);
        let count = all.len();
        if all.len() == 0 {
            log::warn!("Delivery error: no subscribers for {:?}", topic);
            return IpcResult::error(DeliveryError::NoSubscriber.into());
        }
        assert!(
            count == 1,
            "Multiple targets selected for reliable delivery"
        );
        let sub = all.into_iter().next().unwrap();
        if let Some(mailbox) = self.mailboxes.get_mut(&sub).unwrap() {
            // Deliver to another process
            assert!(
                matches!(mailbox.pipe_mode, PipeMode::None),
                "TODO: Error: reply to pipe not allowed"
            );
            let result = mailbox.push_reliable(Message {
                topic: topic.string(),
                data: data.to_vec(),
                ack_id: None,
            });

            match result {
                Ok(trigger) => IpcResult::success(()).with_events(trigger.into_iter()),
                Err(error) => IpcResult::error(error.into()),
            }
        } else {
            panic!("Cannot deliver_reply to the kernel");
        }
    }

    /// Used by kernel to reliably deliver a reply to a delivery.
    /// Acknowledgement to these deliveries must be ignored, as
    /// the reply always succeeds from the viewpoint of the kernel.
    pub fn kernel_deliver_reply<T: serde::Serialize + ?Sized>(
        &mut self, topic: Topic, data: &T,
    ) -> Result<(), DeliveryError> {
        let all = self.subscriptions.find_all(&topic, true);
        let count = all.len();
        if all.len() == 0 {
            log::warn!("kernel_deliver_reply: No subscribers for {:?}", topic);
            return Err(DeliveryError::NoSubscriber);
        }
        assert!(
            count == 1,
            "Multiple targets selected for reliable delivery"
        );
        let ack_id = self.next_acknowledge_id;
        self.next_acknowledge_id = self.next_acknowledge_id.next();
        let sub = all.into_iter().next().unwrap();
        let mailbox = self
            .mailboxes
            .get_mut(&sub)
            .unwrap()
            .as_mut()
            .expect("Kernel cannot reply to itself");

        // Deliver to process, returning any errors to the caller
        assert!(matches!(mailbox.pipe_mode, PipeMode::None));
        let result = mailbox.push_reliable(Message {
            topic: topic.string(),
            data: pinecone::to_vec(data).unwrap(),
            ack_id: None,
        })?;
        assert!(
            result.is_none(),
            "Kernel reply delivery must not cause events"
        );
        Ok(())
    }

    /// Used to see if this is a new delivery or a completed one
    pub fn delivery_complete(&mut self, pid: ProcessId) -> bool {
        self.delivery_result.contains_key(&pid)
    }

    /// Called when process that called deliver wakes up again
    pub fn after_delivery(&mut self, pid: ProcessId) -> IpcResult<()> {
        let result = self
            .delivery_result
            .remove(&pid)
            .expect("No such delivery in after_delivery");
        match result {
            Ok(()) => IpcResult::success(()),
            Err(e) => IpcResult::error(e.into()),
        }
    }

    /// What event this subscription triggers when selected.
    /// Returns WaitFor::None if there are messages available immediately.
    pub fn waiting_for(&mut self, subscription: SubscriptionId) -> WaitFor {
        let mailbox = self
            .mailboxes
            .get_mut(&subscription)
            .expect("Attempt to check_available from an unsubscribed topic")
            .as_mut()
            .expect("The kernel cannot manually check for events");

        mailbox.queue.wait_for()
    }

    /// Read message from a subscription, if any available.
    /// Otherwise return event to wait for.
    pub fn receive(
        &mut self, pid: ProcessId, subscription: SubscriptionId,
    ) -> IpcResult<Result<Message, ExplicitEventId>> {
        verify_owner!(self, pid, subscription);
        let Some(mailbox) = self
            .mailboxes
            .get_mut(&subscription)
        else {
            return IpcResult::error(Error::Unsubscribed);
        };

        let mailbox = mailbox
            .as_mut()
            .expect("The kernel cannot manually receive events");

        IpcResult::success(mailbox.pop_or_event())
    }

    /// Acknowledge reliable delivery.
    /// If positive==false, then negative-acknowledge
    pub fn acknowledge(
        &mut self, _subscription: SubscriptionId, ack_id: AcknowledgeId, positive: bool,
    ) -> IpcResult<()> {
        let (event, pid) = self
            .waiting_for_delivery
            .remove(&ack_id)
            .expect("Attempt to re-acknowledge a message"); //TODO: client error
        self.delivery_result.insert(
            pid,
            if positive {
                Ok(())
            } else {
                Err(DeliveryError::NegativeAcknowledgement)
            },
        );
        IpcResult::success(()).with_event(TriggerEvent(event))
    }

    /// Update when a process completes.
    /// Unsubscribes from all events, cleans mailboxes, and send wakeup signals if required
    pub fn on_process_over(
        &mut self, sched: &mut Scheduler, pid: ProcessId, status: ProcessResult,
    ) {
        // Unsubscribes from all events
        if let Some(subs) = self.process_subscriptions.remove(&pid) {
            for subscription in subs {
                self._force_unsubscribe(subscription)
                    .consume_events(sched)
                    .unwrap();
            }
        }

        // Is this process is connected to any pipes, disconnect them
        // TODO: optimize by caching these when created?
        for (sub_id, mailbox) in self.mailboxes.iter_mut() {
            if let Some(mailbox) = mailbox {
                if let PipeMode::ConnectedTo(target) = mailbox.pipe_mode {
                    if target == pid {
                        mailbox.pipe_mode = PipeMode::Disconnected;
                        if let Some(event) = mailbox.queue.take_event() {
                            sched.on_explicit_event(event);
                        }
                    }
                }
            }
        }
    }
}

lazy_static::lazy_static! {
    pub static ref IPC: Mutex<Manager> = Mutex::new(Manager::new());
}

/// Publish message as the kernel
pub fn kernel_publish<T: serde::Serialize>(sched: &mut Scheduler, topic: &str, message: &T) {
    log::trace!("kernel_publish {}", topic);
    let data = pinecone::to_vec(message).unwrap();
    let mut ipc_manager = crate::ipc::IPC.try_lock().expect("IPC locked");
    ipc_manager
        .publish(Topic::new(topic).expect("Invalid topic name"), &data)
        .consume_events(sched)
        .expect("Publish failed");
}
