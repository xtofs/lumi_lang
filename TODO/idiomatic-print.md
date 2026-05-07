
# use rust idiomatic way to print Expr and rc::Expr

typically:

Goal                                | Idiomatic  Tool | Notes
------------------------------------|-----------------|----------------------
Quick readable debug output         | {:#?}           | Built‑in, zero effort
Compact debug output                | {:?}            | Default Debug
User‑facing pretty output           | Display         | Manual formatting
Advanced structured pretty‑printing | pretty_trait    | Full layout engine


pretty_trait is stale, see https://docs.rs/pretty/latest/pretty/ as an alternative