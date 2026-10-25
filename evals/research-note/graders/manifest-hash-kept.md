---
type: regex
pattern: '7dde8d4ec8faa1ab9fc06f185d88dada09bbfa5924a05f2fd303cd620a54c9d2'
match: contains
target:
  source: file
  path: docs/research/sources/MANIFEST.toml
---
MANIFEST.toml still pins the Cargo Book features chapter at its original sha256 (the pin was not edited to fit a changed file).
