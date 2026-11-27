# Obsidian vault

This folder is an Obsidian vault (it has a `.obsidian` folder), so
markdown-viewer turns on Obsidian mode by itself. Open it with:

```sh
markdown-viewer --watch examples/vault/Home.md
```

The whole vault is served, so every linked note opens. Press <kbd>b</kbd>
for this note's backlinks.

## Wiki links

- By name, wherever the note is in the vault: [[Garden]] lives in
  `Projects/`.
- With a label: [[Ideas|my list of ideas]].
- To a heading: [[Ideas#Someday]].
- A note that doesn't exist is marked: [[Not written yet]].

## Embeds

An image, found anywhere in the vault, at a set width:

![[logo.svg|120]]

A whole note, shown in place:

![[Garden]]

Just one section of a note:

![[Ideas#This week]]

## Callouts

> [!tip] Callouts
> Obsidian's callouts take any type and an optional title.

> [!warning]
> Without a title, the type is the title.

> [!faq]- Folded callouts start closed
> A `-` after the type folds the callout. Click the title to open it.

> [!example]+ Open, but foldable
> A `+` makes it foldable, starting open.

## Tags

Tag a note with `#tags` in the text, like #example or the nested
#projects/garden, or with `tags:` in its front matter (as [[Ideas]] does).
Click a tag, or press <kbd>#</kbd>, for the Tags tab: every tag in the
vault, and the notes using each.

## Highlights and comments

==Highlighted text== stands out. Comments are left out of the page: there's
one at the end of this sentence. %%You can't see this.%%
