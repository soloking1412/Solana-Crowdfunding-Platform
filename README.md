# Solana Crowdfunding

On-chain crowdfunding program. Creators set a goal and deadline, donors send SOL into a locked vault, funds release to the creator if the goal is hit — otherwise donors get refunded.

Deployed on Devnet: `AwYKdLeRXGkxkk2fS2A8AAuYZMFnowkhAvnCuhfByK3L`

---

## Prerequisites

- Rust + Cargo
- Solana CLI (Agave 3.x)
- A funded Devnet wallet

```bash
solana config set --url devnet
solana airdrop 2
```

---

## Build

The project vendors its dependencies (workaround for a platform-tools/edition2024 issue). Just run:

```bash
cargo build-sbf
```

Output lands at `target/deploy/solana_crowdfunding.so`.

---

## Deploy

```bash
solana program deploy target/deploy/solana_crowdfunding.so
```

Save the program ID it prints — you'll need it to derive PDAs and build transactions.

---

## How it works

There are 4 instructions. Instruction data is: 1 discriminant byte + borsh-encoded args.

| # | Instruction | Args |
|---|---|---|
| 0 | `CreateCampaign` | `goal: u64, deadline: i64` |
| 1 | `Contribute` | `amount: u64` |
| 2 | `Withdraw` | _(none)_ |
| 3 | `Refund` | _(none)_ |

**PDAs** (derive these client-side):
- Vault: `["vault", campaign_pubkey]`
- Contribution record: `["contribution", campaign_pubkey, contributor_pubkey]`

### Account order per instruction

**CreateCampaign**
```
0: creator          (signer, writable)
1: campaign account (writable) — new keypair, pre-funded with rent
2: vault PDA        (readonly)
3: system program
```

**Contribute**
```
0: contributor      (signer, writable)
1: campaign account (writable)
2: vault PDA        (writable)
3: contribution PDA (writable)
4: system program
```

**Withdraw**
```
0: creator          (signer, writable)
1: campaign account (writable)
2: vault PDA        (writable)
3: system program
```

**Refund**
```
0: contributor      (signer, writable)
1: campaign account (writable)
2: vault PDA        (writable)
3: contribution PDA (writable)
4: system program
```

---

## Rules

- Withdraw: caller must be creator, deadline must have passed, raised >= goal, not already claimed
- Refund: deadline must have passed, raised < goal
- Each contributor's amount is tracked separately — partial refunds work correctly
- Contribution account is closed on refund (rent returned to contributor)

---

## Errors

| Code | Meaning |
|---|---|
| 0 | Deadline is in the past |
| 1 | Deadline hasn't passed yet |
| 2 | Goal not reached (withdraw attempt) |
| 3 | Goal was reached (refund attempt) |
| 4 | Already claimed |
| 5 | Not the campaign creator |
| 6 | Wrong vault address |
| 7 | No contribution found |
