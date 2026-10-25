---
type: regex
pattern: '\[crates\.rust-code-domain\]\nseam = 8\n#[^\n]*\nas_cast = 2\n\n\[crates\.rust-code-core\]'
match: contains
target:
  source: file
  path: budgets.toml
---
The domain crate's budgets in budgets.toml are unchanged (seam = 8, as_cast = 2, and no other kind given a budget).
