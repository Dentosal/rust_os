# Bash spells

## Parse bochs `show call` output to function calls

```
grep 'call 0x' b.log | sed 's/.*call //' | cut -d' ' -f 1 | sed 's/0x0008:0*//' | xargs -n 1 -I {} grep --color=always -B2 {}: k.elf
```

Skip duplicates

```
grep 'call 0x' b.log | sed 's/.*call //' | cut -d' ' -f 1 | sed 's/0x0008:0*//' | uniq | xargs -n 1 -I {} grep --color=always -B2 {}: k.elf
```
