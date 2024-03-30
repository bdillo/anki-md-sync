# anki-md-sync

A simple tool to write up notes in Markdown and sync them to Anki. The Markdown format by default looks like:

```markdown
---
deck: MyDeckName
---

Q: What size is rust's `char` type?
A: 4 bytes

Q: What is the block size of AES?
A: 128 bit
```

The top section (delimited by `---`) is for metadata, currently the only field supported is setting the Anki deck you
want to push the cards into.

The front of each card starts with "`Q: `", and the back starts with "`A: `". The questions and answers are converted to
HTML and pushed into Anki using [AnkiConnect](https://foosoft.net/projects/anki-connect/), which is required for this
tool to work.

The tool is very simple and relies on AnkiConnect to not duplicate cards, it keeps no state so each time it is run it
will try to add a new note for every Q/A pair in the Markdown file(s).

## Example
```shell
anki-md-sync -f AnkiFile1.md AnkiFile2.md
[2024-03-30T02:27:44Z INFO  anki_md_sync] Syncing file "AnkiFile1.md"...
[2024-03-30T02:27:44Z INFO  anki_md_sync] Done!
[2024-03-30T02:27:44Z INFO  anki_md_sync] Syncing file "AnkiFile2.md"...
[2024-03-30T02:27:44Z INFO  anki_md_sync] Done!
```