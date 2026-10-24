# The judge protocol

How to get a decision from a judge -- an LLM subagent, an external judge
model, or a human reviewer -- that carries signal rather than echoing the
asker. Back to [SKILL.md](../SKILL.md).

## When a judge is the right tool

- Use a judge only when evidence does not settle the question: two
  CONFORMING options with different costs, a readability or visual question,
  a trade-off the governing text is silent on.
- Do not ask a judge what a quote or a measurement can answer -- quote or
  measure. A judge's preference is weaker evidence than either.
- A judge cannot make a DEPARTS option conforming. It can only support
  choosing it as a recorded departure (a D-n naming the text), which the
  owner may veto.

## What "uncached" means per judge

The two asks must be independent: the second must not see, remember or be
served the first.

| judge | independent second ask |
|---|---|
| LLM subagent | a fresh subagent with no transcript, no shared memory, never the agent that built the option; nothing from ask 1 in its prompt |
| external judge model | a separate call with no shared conversation; any response cache (in the judge library or a proxy) bypassed -- a cached reply is ask 1 again |
| human | a second reviewer, or the same reviewer without access to their first answer; options relabelled as well as reordered |

## The packet

Everything the judge sees, and nothing else:

1. **The question**, as one neutral sentence.
2. **The options**, labelled neutrally (`X`, `Y`), described at the same
   length and level of detail. A longer description reads as the favoured one.
3. **The principles**, quoted verbatim with their `[[source: <key>]]` and
   clause -- the judge rules against stated principles, not its own taste.
4. **The evidence**, each item with its label (measured / by-reading /
   inferred / unmeasured). If one option has a measurement and the other has
   none, say so; never show numbers for one side only.
5. **Renders** for any visual question: both options at the same size, scale,
   theme and conditions, labelled with the same neutral labels. A judge cannot
   rate what it is only told about.
6. **The return format** (below): a choice, a confidence and the principle
   that decided it.

## Leaked framing voids support

Framing that tells the judge which answer is wanted makes its agreement
worthless. Leaks to check for before sending:

- names that grade: "proper fix", "hack", "workaround", "new", "legacy";
- telling the judge which option is already built, or whose it is;
- the asker's or the owner's leaning, even as "we think";
- asymmetric detail, or evidence for one side only;
- an order that tells a story (problem, then the "solution").

If a leak is found after the ask, the ask is **void**: record "void: framing
leaked" with the leaking phrase quoted, and ask again with a clean packet. A
void ask does not count toward the two.

## Procedure

1. **Ask 1**: options in order `X, Y`.
2. **Ask 2**: independent (table above), options in order `Y, X`.
3. **Accept** only if both asks pick the same option **and** both report
   confidence >= 0.9.
4. **Position check.** If the asks pick different options and each picked
   the one in the same position, the judge is answering the order, not the
   question: no signal. Reordering is what exposes this.
5. **Otherwise, research, then re-ask** (both orders again, independent
   again). First diagnose why it fell short: a missing fact or measurement,
   the framing, a stale packet, or a pure preference. Then research what is
   missing -- primary text, a measurement, a render -- or reframe, and state
   in the note what was added. What is added is facts, never argument for an
   option. Never re-send an unchanged packet: a low score means the question
   was unclear, and re-sending until it agrees is sampling for the answer you
   want. Repeat while research can still add a fact.
6. **The owner** gets only what no fact settles: a pure preference, or a
   question still below the bar once research has nothing left to add. Send
   every round's figures and packets, and the option you recommend and why.
   A preference goes to the owner directly; a judge's pick there is noise.

## Return format

Ask for data, so the figures go into the note unedited:

```json
{
  "choice": "X",
  "confidence": 0.93,
  "principle": "<key> clause N",
  "reason": "one or two sentences, citing the principle",
  "missing": "what would have raised confidence, or empty"
}
```

`missing` feeds the re-ask in step 5: it is the judge saying what context
the packet lacked.

## Ask template

```text
You are judging a design question. Answer from the principles and evidence
below only. You have not seen this question before.

Question: <one neutral sentence>

Principles (verbatim, with source and clause):
- "<quote>" -- [[source: <key>]] <clause>

Evidence (each labelled measured / by-reading / inferred / unmeasured):
- <item> (<label>)

Options (the order is arbitrary):
- <label 1>: <description, same length as the other>
- <label 2>: <description>

Renders, if any: <attached files, labelled with the option labels>

Return only this JSON: {"choice": "<label>", "confidence": <0..1>,
"principle": "<key> <clause>", "reason": "...", "missing": "..."}
```

## A judge that returns only probabilities

Some judges answer with probabilities alone: a yes/no question returns the
probability that its statement is true, and a choice returns a probability
per option, perhaps with a confidence, but no `principle`, `reason` or
`missing`. The procedure above still holds, with these changes:

- **Ask direct factual yes/no questions, one judgment each, and combine them
  in code.**
  Write the rule that turns the answers into a decision before the first
  ask, and record it with the figures. A choice between options that argue
  for themselves follows whichever argues best, so a choice goes only over
  neutral options of equal length.
- **Settle by reading what reading settles.** With one such judge (inferred
  from a few dozen asks, not measured as a rate), answers read the text
  literally and moved with the order it was shown. A fact you can check in the
  text is yours to check: label it by-reading, with the line. Ask the judge
  only for the judgment, and send it what you read as a quote.
- **Send the material whole** as named fields: the files, the command
  output, the owner's words verbatim and dated, the earlier answers. Never
  send your summary of them, and keep conclusions out of the answer
  criteria.
- **Read the bar on confidence.** For a choice that reports a confidence,
  that confidence must be >= 0.9 on both asks, not just the top option's
  probability. A yes/no answer's confidence is |2p - 1|, the same margin a
  two-option choice reports: it must be >= 0.9 (p >= 0.95 or p <= 0.05) on
  the same side, both times.
- **Find what is missing yourself.** No `missing` field comes back. Find
  which answer fell short and research that fact (step 5). A companion
  question, "does the state hold what this question needs?", can point to
  the gap.
- **Check from the log that both asks were uncached.** A client may cache on
  a hash of the request, and a hash over sorted keys ignores reordering. So
  make ask 2's body differ (reorder, and add a neutral sentence), and check
  the judge's own log that neither answer was served from cache. A cached
  reply is ask 1 again.
- **Never a gate.** Answers move between identical asks and between judge
  versions. A judge informs a decision a human can veto; it never makes a
  build pass or fail. Pin the judge's version in the note, not an alias.
- **Opt-in, with a keyless default.** The default judge is a fresh LLM
  subagent or a human, and neither needs anything installed. An external
  judge is the project's own opt-in: its client and its key live outside
  the repository, and no gate or eval depends on it. The note's judge column
  names which judge answered.

## Recording the figures

In the note's "How it was checked":

| round | ask | order | judge | choice | confidence | principle | void |
|---|---|---|---|---|---|---|---|
| 1 | 1 | X, Y | `<judge>` | X | 0.93 | `<key>` clause N | no |
| 1 | 2 | Y, X | `<judge>` | X | 0.91 | `<key>` clause N | no |

Then the outcome: accepted, sent to the owner, or void. The decision's
**Evidence** cites the note, not the judge directly.

## The owner

- The owner is the last step of the protocol and holds a **veto** over any
  judge result, refuter verdict or measurement-backed proposal.
- Quote the owner verbatim, dated, in the note or the decision:
  `Owner, YYYY-MM-DD: "<exact words>"`. A paraphrase drifts toward what the
  writer wanted to hear, and cannot be checked later.
- The owner's word reopens a decision as easily as it closes one: an owner
  remark that contradicts a recorded decision is new evidence -- record it and
  re-research, do not argue it away.

The packet, the procedure and the figures live in the repo,
and the judge is pluggable -- the protocol is the same for every kind.
