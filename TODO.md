# 🚀 TODO.md – GhostBridge Cross-Chain Bridge MVP (ETH, Stellar, Ghostchain)

> This document scopes the next stage of GhostBridge: evolving from a DNS/blockchain gRPC bridge into a fully cross-chain bridge for Ghostchain, Ethereum, and Stellar.  
> Focus: Asset bridging, identity resolution, domain lookup, and future-proof extensibility.

---

## 📅 Milestone 1: Protocol & Service Definitions

- [ ] **Extend proto/ definitions:**  
    - Add `eth_bridge.proto` and `stellar_bridge.proto` for Ethereum/Stellar-specific RPC.
    - New universal RPCs:  
        - `CrossChainTransfer`
        - `CrossChainIdentity`
        - `CrossChainDomainLookup`
    - Add `chain_id` or `network` parameter to all chain-aware RPCs.
- [ ] **Document all new service APIs and message types.**
- [ ] **Generate new Rust/Zig gRPC bindings.**

---

## 📅 Milestone 2: Zig Bridge Server Extension

- [ ] **Add Ethereum and Stellar handler modules:**  
    - `zig-server/src/eth_bridge.zig`
    - `zig-server/src/stellar_bridge.zig`
- [ ] **Stub implementations for core cross-chain RPCs:**  
    - Accept transfer/lookup requests; return dummy or testnet responses for now.
- [ ] **Integrate chain routing:**  
    - Route each request to the correct backend (Ghostchain, ETH, Stellar).
    - Support gRPC streaming for event/log watching.

---

## 📅 Milestone 3: Rust Client & Chain Integration

- [ ] **Add Rust modules for Ethereum and Stellar:**  
    - `rust-client/src/eth_bridge.rs`
    - `rust-client/src/stellar_bridge.rs`
- [ ] **Integrate open source crates:**  
    - Use `ethers-rs` for ETH, `stellar-sdk` for Stellar test calls.
- [ ] **Connection pooling and async calls for all chains.**
- [ ] **Implement minimal demo for cross-chain asset transfer and domain lookup.**

---

## 📅 Milestone 4: Identity & Domain Unification

- [ ] **Unified identity resolution API:**  
    - Returns all known identities (DID, ETH, XLM, GhostID) and proofs.
- [ ] **Unified domain lookup:**  
    - Accepts `.ghost`, `.eth`, `.xlm` and returns result from appropriate chain.
- [ ] **Linking API:**  
    - RPC to associate addresses/identities across chains.

---

## 📅 Milestone 5: Security, Audit, and ZKP

- [ ] **Log/sign all cross-chain operations.**
- [ ] **Sign responses with bridge node key (use shroud for Ed25519/Schnorr).**
- [ ] **Prepare stubs for ZKP-based audit in future versions.**

---

## 📅 Milestone 6: Testing, Demo, and Docs

- [ ] **Integration tests:**  
    - gRPC roundtrip for ETH, Stellar, Ghostchain.
    - CLI for asset transfer, domain/identity resolution.
- [ ] **Update deployment scripts and docker-compose.**
- [ ] **Write `EXPLAIN.md` or inline docs for all new flows.**

---

## 🏆 Stretch Goals / Future-Proofing

- [ ] MASQUE/HTTP3 relay support
- [ ] zk-bridge prototype (future)
- [ ] Add Cosmos, Polkadot, Solana, or other chains  
- [ ] Cross-chain governance hooks
- [ ] Web UI / dashboard for bridge monitoring

---

> **Tip:** Ship a single, minimal cross-chain asset transfer demo ASAP—even if it’s testnet or “dummy” only.  
> Focus on code cleanliness, modularity, and full gRPC/proto roundtrip between Zig server and Rust clients.

---

**Hand-off:**  
Claude, use this TODO.md as a working backlog—open issues for each subtask and mark complete as you go!

