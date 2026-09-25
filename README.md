# toil

> **<u>t</u>able <u>o</u>utput/<u>i</u>nput <u>l</u>ibrary**

## about
`toil` is a rust library for writing padded, `#`-commented text tables, whole or a block at a time, and for reading them back.

```text
# step     cmd   job wall(s) max_rss exit status argv
# -------- ----- --- ------- ------- ---- ------ ----
[1](setup) mkdir -   1.50    2.00MiB 0    ok     /mkdir -p out
[2](burn)  a     1   1.50    2.00MiB 0    ok     /a --jobs 4
```

The library is purpose-built for my own use cases, and it was primarily developed using Claude code.
