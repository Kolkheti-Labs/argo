# M1 decisions

Where M1 differs from the M0 spec (`docs/spec/state-layout.md` at tag `m0-r1`, reviewed in M0), and why. Anything
not listed here follows the spec; the spec items M1 did not implement are under "Not implemented from the M0 spec".
Where a source says Logos, it is the RFP-008 Discord thread unless a link is given.

Pins: LEZ `v0.2.4` (47eba256), SPEL rev `1ef0500f` (upgrade to v0.7.0 deferred to M2, where the RFP-001 admin
library needs it).

## Needs Logos input

1. The rule for choosing `max_age_ms` (D5, see
   [below](#d5-and-f11-why-the-core-checks-staleness-and-how-to-choose-max_age_ms)), and how often each testnet
   price feed updates, so we can set defaults.
2. Whether a price account could carry its source's observation time, so the maximum-age check also works for TWAP
   feeds (see "What the bound measures" below).

## Summary

| # | Topic | M0 spec | M1 | Source |
| --- | --- | --- | --- | --- |
| [D1](#d1-token-program) | Token program | v0.2.4 builtin token | lez-programs v2.0.1 token, pinned in one adapter module | lez-programs `DEPLOYMENTS.md` |
| [D2](#d2-oracle-parameter) | Oracle parameter | a program id | a digest of the oracle program, price account and `max_age_ms` | Logos (sasha) |
| [D3](#d3-price-encoding) | Price encoding | `U256`, scale 1e36 | `u128` Q64.64, raw loan units per raw collateral unit | Logos (sasha); stablecoin README §4.5 |
| [D4](#d4-pair-binding) | Pair binding | not specified | the price account's pair must be the market's two tokens | stablecoin README §4.5 |
| [D5](#d5-staleness) | Staleness | none in core | core rejects a zero price, and a price older than `max_age_ms` (1 minute to 1 day) | Logos (sasha) |
| [D5a](#d5a-future-dated-prices) | Future-dated prices | a bounded skew, no value set | reject a price more than 180 s ahead of the clock | our call; Logos (sasha) agreed |
| [D6](#d6-price-account-decode) | Price-account decode | check a layout tag | there is no tag; id, owner, decode and pair checks instead; trailing bytes ignored | Logos (sasha); RFP-019 |
| [D7](#d7-feed-existence) | Feed existence | not specified | `create_market` requires the price account to exist | our call |
| [D8](#d8-time) | Time | seconds | milliseconds from `CLOCK_01` | Logos (sasha) |
| [D9](#d9-irm) | IRM | compiled in | `MarketParams.irm` is a program id; M1 ships accrual scaffolding only | RFP-008 F8, proposal §5, issue 172 |
| [D10](#d10-fee-fields) | Fee fields | fee in Config | `fee` and `pending_fee_shares` in Market, both zero in M1 | M2 fee design |
| [D11](#d11-dead-deposit) | Dead deposit | not specified | `create_market` locks 1_000 raw loan units that nobody can withdraw | proposal §3 |
| [D12](#d12-same-token) | Same token | open question (§7 item 10) | rejected | M1 design check |
| [D13](#d13-clock-source) | Clock source | `CLOCK_01` | unchanged; checked by id, a zero timestamp is rejected | M0 spec §5 |
| [D14](#d14-initial-admin) | Initial admin | first signer of `initialize` | a fixed admin id in the build; M1 uses a public test key | M1 design check; M1 review |
| [D15](#d15-inputs-the-runtime-cannot-check) | Inputs the runtime cannot check | not specified | refused; callers and the admin must be program-owned | M1 review |
| [D16](#d16-morpho-callbacks) | Morpho callbacks | open question (§7 item 9) | not implemented | spec §7 item 9 |

## Details

### D1. Token program

M0 spec: the v0.2.4 builtin token (`programs::token()`, `token_core`).

M1 uses the lez-programs `v2.0.1` token, image id `14b3bc6cc129d0359f9595ba0f60a13e4a8df08ffe8eb1a83251c47abecc67a8`,
pinned as a constant in one adapter module. Its Borsh types are copied there (its `TokenDefinition::Fungible` has an
extra `authority` field, so the builtin decoder rejects its definitions). The M0 spikes keep the builtin.

Source: lez-programs `DEPLOYMENTS.md` on `main` (testnet-legacy, "built from tag v2.0.1"); testnet-legacy faucet
tokens are owned by `14b3bc6c`, not the builtin.

### D2. Oracle parameter

M0 spec: `MarketParams.oracle` is a `ProgramId`.

M1: a 32-byte digest `SHA-256("argo/oracle/v1" ‖ borsh(OracleDescriptor))`, where
`OracleDescriptor = { version: u8, oracle_program: ProgramId, price_account: AccountId, max_age_ms: u64 }`. The
descriptor is stored in Market. The market-id preimage stays 158 bytes. `oracle_program` may not be the default
(unowned) program id.

Source: Logos (sasha) accepted `hash(program id, price account id, maxAge)` with the GUI showing the inputs.

### D3. Price encoding

M0 spec: `U256`, scale 1e36.

M1: `u128` Q64.64, meaning raw loan-token units per one raw collateral-token unit
(`twap_oracle_core::PRICE_FRACTIONAL_BITS = 64`). `ORACLE_PRICE_SCALE` (1e36) is removed. Collateral value =
`collateral × price / 2^64` with a wide intermediate, rounded down. The encoding fixes the range: a raw/raw price
must be below 2^64, a price below 2^-64 becomes 0 and is refused as zero, and very small prices are coarsely
quantised (spec §5 raised the u128 range question). Kanon, the RFP-020 RedStone adaptor, currently writes
`(median << 64) / 10^decimals`, a price per whole base asset (Kanon ADR 17), so whether a given Kanon feed is raw/raw
for a market's two tokens is checked when M4 wires real feeds.

Source: Logos (sasha): every `OraclePriceAccount` producer, RedStone included, must write this encoding;
lez-programs stablecoin README §4.5 on `main` ("Every producer must use this encoding").

### D4. Pair binding

M0 spec: not specified.

M1: the price account must decode as `OraclePriceAccount` with `base_asset = collateral_token` and
`quote_asset = loan_token`. The pair is checked against the market's tokens, not stored twice.

Source: stablecoin README §4.5 (the same check, with its own orientation).

### D5. Staleness

M0 spec: no staleness check in core (RFP-008 Reliability preamble).

M1: core rejects a price when `now_ms − price.timestamp > max_age_ms` and when `price == 0`. `max_age_ms` must be in
`[60_000, 86_400_000]` (one minute to one day). This is a recorded deviation from the preamble, justified against F11
[below](#d5-and-f11-why-the-core-checks-staleness-and-how-to-choose-max_age_ms).

Source: Logos (sasha) accepted it: "justify the maxAge values against the strand-collateral risk in F11". The
one-day ceiling is from stablecoin README §8.

### D5a. Future-dated prices

M0 spec: §7 item 2 tolerates a price later than the clock "up to a bounded skew", with no value set.

M1: core also rejects a price stamped more than `MAX_FUTURE_SKEW_MS` = 180 s after `now_ms`. Without it, a price
dated in the future does not age until the clock reaches its timestamp, so it stays fresh well past `max_age_ms`
(forever if dated far enough ahead), which defeats D5. 180 s is Kanon's forward tolerance: it accepts a RedStone
package dated at most 180 s ahead of `CLOCK_01` (Kanon ADR 18), and the price account's timestamp is that round time
(Kanon ADR 22). So a price Kanon accepted is not rejected by this check, unless `CLOCK_01` moved backwards between
Kanon's write and Argo's read (v0.2.4 does not enforce increasing block timestamps, spec §5).

Source: our call, found in M1 review. Logos (sasha) agreed to keep it.

### D6. Price-account decode

M0 spec: §7 item 2 checks the price account's id, owner and "a layout tag".

M1: `OraclePriceAccount` has no layout tag; the id, owner, decode and pair checks (D4) stand in for it. Decode with
`BorshDeserialize::deserialize(&mut slice)`, ignoring trailing bytes, so a field appended later under RFP-019's
append-friendly standard does not break markets that exist. A deployed core's code cannot change and it accepts only
descriptor `version` 1, so reading a new field needs a new build of Argo, deployed under a new program id; markets on
the deployed core never start reading it.

Source: Logos (sasha): existing markets keep what they committed to; RFP-019 append-friendly evolution.

### D7. Feed existence

M0 spec: not specified.

M1: `create_market` requires the price account to exist, be owned by `oracle_program` and decode with the right pair.
An immutable market bound to a missing feed cannot be repaired.

Source: our call. The stablecoin allows a feed that does not exist yet because its admin can rotate it; Argo has no
rotation.

### D8. Time

M0 spec: `now = timestamp / 1000`, `last_update` in seconds.

M1: milliseconds throughout: `last_update_ms`, and `now_ms` from `CLOCK_01`. Rates will stay per second (Morpho
constants); M2's accrual will use `rate × elapsed_ms / 1000` through `mul_div`, so sub-second elapsed time is not
truncated before the multiply (the single division at the end rounds down). All time reads go through one function,
so a caller-passed `now` with a validity window can replace it later.

Source: Logos (sasha): timestamps in ms from `CLOCK_01`.

### D9. IRM

M0 spec: IRMs compiled into Argo (proposed in §3 and §7 item 13).

M1: `MarketParams.irm` is a `ProgramId`; Config holds the enabled set. M1 ships the accrual scaffolding only
(`last_update_ms` moves forward, never back; the rate is 0). In M2 a market whose IRM is the pinned `argo_irm` id runs
`irm_core` in-frame; the rate model, the standalone `argo_irm` program and the module-vs-accrual parity test are M2.

Source: RFP-008 F8, proposal §5, issue 172; Logos (sasha) on F8 reads.

### D10. Fee fields

M0 spec: a fee fraction in Config (§7 item 5).

M1: Market holds `fee` and a reserved `pending_fee_shares`, both zero in M1, so the layout does not change in M2.

Source: the M2 fee design (fee shares accrue in Market, with a permissionless claim).

### D11. Dead deposit

M0 spec: not specified.

M1: `create_market` takes `DEAD_DEPOSIT` (1_000) raw loan units from the creator's holding into LoanVault, credited as
supply shares to `Position(market, dead_owner(market))`, where `dead_owner = SHA-256("argo/dead-owner" ‖ market_id)`:
an id no key and no program can authorise, so nobody can withdraw it. The creator pays.

Source: proposal §3 ("a small dead deposit at market creation").

### D12. Same token

M0 spec: open question (§7 item 10: forbid it, or support it).

M1: `create_market` rejects `loan_token == collateral_token`. This is a recorded deviation from Morpho, which allows
it. In practice the runtime already rejects it, because the two definition inputs would be the same account.

Source: M1 design check.

### D13. Clock source

M0 spec: `CLOCK_01`.

M1: unchanged. `CLOCK_01` is asserted by id (a fixed genesis id no key or program can derive), and `timestamp == 0`
is rejected. D15 makes a forged clock impossible.

Source: M0 spec §5.

### D14. Initial admin

M0 spec: the first signer of `initialize`.

M1: `initialize` accepts only `ADMIN_GENESIS`, a constant in the image (deployment has no signer, so otherwise anyone
could initialise first). M1 uses the public test key `[1; 32]`; a real key or RFP-001 nominate/accept (M2) replaces
it before any public deployment. A fresh admin account is claimed by Argo (as M0 did) so it stays checkable; a
program-owned admin account is left alone; an unowned account that has already signed anything is refused (it can
never be claimed or checked). So the real admin key must either sign nothing before `initialize` or already be
program-owned.

Source: M1 design check; M1 review.

### D15. Inputs the runtime cannot check

M0 spec: not specified.

M1: every handler refuses an input that is owned by the default program id and is not the default account. SPEL's
generated `main` (rev 1ef0500) drops such inputs from the output, and LEZ v0.2.4 checks state and authorisation only on
the pre-states a program outputs (`lee/state_machine/src/validated_state_diff/mod.rs:143-173`), so a calling program
could forge them with any content and any `is_authorized`. Consequences: a position owner acting in person (`caller`)
must be a program-owned account, such as a token holding; the admin must be program-owned after `initialize`; payer
and receiver holdings must be holdings of the market's token owned by the pinned token program.

Source: M1 review (forged-caller theft path through a wrapper program).

### D16. Morpho callbacks

M0 spec: open question (§7 item 9: drop the callbacks in M1 and record the deviation, or add them).

M1: not implemented. There is no `onMorphoSupply` / `onMorphoRepay` / `onMorphoSupplyCollateral` / liquidation
callback. A liquidator or repayer holds the loan tokens up front.

Source: spec §7 item 9.

## D5 and F11: why the core checks staleness, and how to choose `max_age_ms`

RFP-008 F11 rules out an emergency pause because pausing a market with active borrows would strand collateral
and stop liquidations. A staleness check is a narrow, automatic halt of the same kind, so it has to be weighed
against that failure mode.

What a stale price blocks in Argo: `borrow`, `withdraw_collateral` while the position has debt, and (from M3)
`liquidate`. What it never blocks: `supply`, `withdraw`, `supply_collateral`, `repay`, and `withdraw_collateral`
with no debt. So a stale price never stops a lender leaving (liquidity permitting) or a borrower who repays in full
and then withdraws (both subject to the M1 caller rule under "Still open"). It strands collateral only for a
borrower who cannot repay in full, and only while the feed is stale; from M3 it also pauses liquidations in that
market for the same time.

The trade-off. A LEZ price account is passive data: it cannot refuse to be read, and a market's price account is
fixed for life (a Morpho oracle, by contrast, is a contract: it can revert, and depending on its implementation it can be re-pointed). Without
a bound, a feed that stops publishing strands nothing, but the market keeps acting on a frozen price indefinitely:
borrowers can borrow against a collateral price that is no longer true (lenders' loss), and from M3 healthy
positions could be liquidated, or unhealthy ones left alone, at that price. With the bound, the market stops acting
on that price after `max_age_ms`, at the cost of stranding the collateral of borrowers who cannot repay in full
until the feed resumes. We prefer the bound: it caps how long a market can act on a price whose timestamp has stopped
advancing, and a stale price never stops a borrower who can repay in full.

What the bound measures. Argo computes age as `now_ms − timestamp`, where `timestamp` is whatever the producer
stored, so its meaning depends on the feed:

- `twap_oracle` (lez-programs v2.0.1, the feed the sequencer e2e uses): `PublishPrice` needs no signer and stamps
  `timestamp = now` from `CLOCK_01`; by design a price that has not changed still gets a fresh timestamp
  (`programs/twap_oracle/src/publish_price.rs`). So for a TWAP feed the bound only shows that someone published
  recently. It cannot tell that the source behind the TWAP has stopped reporting: that time
  (`CurrentTickAccount.last_updated`) is not in the `OraclePriceAccount`, and Argo reads only that account. Logos's
  RFP-019 review raised the same concern (M1-019-04, logos-co/rfp issue 215) and accepted extrapolating the
  published timestamp to the live clock as the fix; extrapolation keeps the timestamp fresh but does not show
  whether the source is still reporting. We will ask Logos whether the price account could carry the source's
  observation time, which markets created on a new Argo build (a new program id, D6) could then commit to reading.
- Kanon (the RFP-020 RedStone adaptor, branch `m3-delivery`): the timestamp is the RedStone round's observation
  time, not the write time (Kanon ADR 22), and Kanon accepts a round that is already up to its per-feed `max_age`
  old when it writes it (300 s on its testnet feeds, Kanon `TESTNET.md`). Here the bound does measure the source's
  age, but a price can already be minutes old when it lands.

Choosing the value. `max_age_ms` should be long enough that only a feed that has really stopped trips it, and no
longer:

- `max_age_ms ≥ 2 × the feed's update interval + its longest observation-to-write delay + one block interval`.
  Two intervals tolerate one missed update. The delay is 0 for TWAP and up to the feed's own `max_age` for Kanon.
  The block term covers Argo seeing time only once per block (`CLOCK_01` holds the previous block's time, spec §5).
- Update interval: for TWAP, the publishing cadence the market creator relies on (not the TWAP window); for Kanon,
  the relayer's heartbeat, which Kanon has not fixed yet (its M4).
- Floor 60 s, about one testnet-legacy block (consecutive block timestamps 60.1 to 60.2 s apart, read over RPC on
  2026-11-12). It is only a sanity bound: at the floor, a `CLOCK_01`-stamped price is fresh only in the block it
  was written. Usable values come from the rule above.
- Ceiling one day: past that a price says little about the market (the stablecoin's bound, stablecoin README §8).

The value is fixed per market at creation and is part of the market id (D2), so lenders judge it with the oracle.
Concrete values per feed are set when M4 wires real feeds. In-process tests use a mock-owned price account; the
sequencer e2e uses a `twap_oracle` v2.0.1 price account. This section is our proposal and still needs Logos's
review.

## Not implemented from the M0 spec

- §3 "Credited holdings must exist" allowed a receiver to be an authorised default account as well as a holding
  of the market's token. M1 accepts only a holding owned by the pinned token program (D15).
- §3 aliasing: the spec proposed one input serving two roles. Not implemented: a transaction may not list an
  account twice, so the `caller` cannot also be the receiver; the user names a second holding as receiver.
- §7 item 2 (check "any confidence field" on `borrow` and on `withdraw_collateral` with debt): not implemented; `confidence_interval` is decoded but not
  checked. Whether and how to use it is decided per feed in M4.
- §7 item 11 (stop claiming the admin's account): not adopted; see D14.
- §7 item 12 (`overflow-checks = true` in the guest release profile): not adopted. Clippy's
  `arithmetic_side_effects = "deny"` (root `Cargo.toml`) rejects unchecked arithmetic in Argo's workspace
  crates, apart from documented exemptions: in non-test code, `argo_core::math`'s widening multiply and long
  division and the const seed padding in `argo_lending_core` (and the same padding in the M0 spike crate); test
  modules are exempt too. CI runs clippy with `-D warnings`. It does not cover
  dependencies or the guest dispatcher crate, which `overflow-checks` would. The item's `min_seized` /
  `max_repaid` bounds belong to liquidation (M3). Its per-operation rounding table was not added to spec §5; the
  directions are in `ops.rs` and `argo_core::health`, and traceability rows F4 to F7 cite the tests.

## Still open

- `on_behalf` must equal the signing `caller` for withdraw, borrow and withdraw_collateral; manager authorisation
  (F17) is M3. A position whose owner is not a program-owned account (for example a plain key that is not a token
  holding; a key that has already signed while unowned can never become one, D14) can be supplied to, but its
  owner cannot withdraw, borrow or withdraw collateral until M3.
- RFP-008 F11 asks for an LLTV *removal* function on testnet; M1 has `enable_lltv` only (admin scope is M2).
- TWAP feeds and staleness: see "What the bound measures" above; the question to Logos is open.
