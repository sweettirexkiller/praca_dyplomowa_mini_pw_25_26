# Plan: Testing & Writing "Analysis of the Solution"

**Goal:** Conduct all tests, collect data, and write Chapter 3 using results.  
**Timeline:** 9 days (2 hours/day) — 5 days testing, 4 days writing.

---

### PART 1: Conducting Tests (Days 1–5)

#### **Day 1 — Setup & Unit Tests (2h)**

- **Document testing environment (30 min):**
    - Record hardware specs (CPU, RAM, OS).
    - Run `iperf3`/`ping` between machines.
    - Note LiveKit Docker version, `rustc --version`.
    - Take screenshots.
- **Extend unit tests (60 min):**
    - Add tests in `automerge_backend.rs`:
        - Concurrent draw from two peers (verify both strokes after sync) → NF-06
        - Clear + concurrent draw (add-wins) → NF-06
        - Load corrupted/invalid bytes (graceful error)
        - 3-peer sync chain (A→B→C) for multi-peer convergence
- **Run all tests, capture output (30 min):**
    - `cargo test 2>&1 | tee test_output.txt`
    - Screenshot terminal

---

#### **Day 2 — Performance Instrumentation (2h)**

- **Add timing code (90 min):**
    - Benchmark `.crdt` load time: generate files with 100/500/1000/5000 strokes, measure `backend.load()` with `std::time::Instant` → NF-08
    - FPS logging in `update()` (use egui's frame time or manual timing) → NF-10
    - Timestamp logging at `generate_sync_message` (send) and `receive_sync_message` (receive) → NF-07
- **Verify build (30 min):**
    - `cargo build --release`
    - Quick smoke test

---

#### **Day 3 — Performance Testing: Latency, File Load, FPS (2h)**

- **Sync latency — NF-07 (45 min):**
    - 2 clients, same room. Draw on A, measure time to appear on B.
    - 30–50 trials. Record min/max/mean/p95/stddev in CSV.
- **File load time — NF-08 (30 min):**
    - Run benchmarks. Record: stroke count | file size (KB) | load time (ms). 5 trials per size.
- **Rendering FPS — NF-10 (45 min):**
    - Release build. Measure FPS with 0, 100, 500, 1000 strokes. Record avg/min FPS.

---

#### **Day 4 — Scalability, Chunking, Reliability (2h)**

- **Multi-user scalability — NF-09 (45 min):**
    - Connect 2/3/4/5 clients. Each draws for 30s.
    - Record: num clients | avg latency | max latency | CPU % | RAM (MB).
- **Chunking overhead (30 min):**
    - Send payloads of ~1KB/10KB/50KB/200KB. Measure delivery time. Verify chunking above 14KB.
- **CRDT convergence — NF-06 (30 min):**
    - Two clients draw simultaneously for 60s. Save both `.crdt` files, compare stroke lists (must match). Repeat with clear-during-draw.
- **Offline continuity — NF-05 (15 min):**
    - Draw while disconnected (kill LiveKit Docker). Reconnect. Check if offline strokes sync. If not, document as limitation.

---

#### **Day 5 — Manual Acceptance Tests & Usability (2h)**

- **Execute MAN-01 through MAN-10 (75 min):**
    - Follow `TESTING_MANUAL.md` step by step. Record pass/fail + screenshots.
- **Usability assessment — NF-01 to NF-04 (30 min):**
    - Screenshots: startup UI, connection status, remote cursors, English interface.
    - Optionally, have 1–2 people try for 10 min.
- **Organize all data (15 min):**
    - Store CSVs, screenshots, notes in `DYPLOM/2. thesis/images/` or `test_results/`, named by section.

---

### PART 2: Writing Chapter 3 (Days 6–9)

#### **Day 6 — Sections 3.1, 3.2, 3.3 (2h)**

- **3.1 Testing environment (30 min):**
    - Hardware, network, LiveKit config, Rust toolchain.
    - Small summary table.
- **3.2 Unit testing (45 min):**
    - Results table (test name | what it verifies | pass/fail).
    - Include `cargo test` screenshot. Discuss coverage.
- **3.3 Manual acceptance testing (45 min):**
    - Results table for MAN-01..MAN-10.
    - 3–4 representative screenshots.

---

#### **Day 7 — Section 3.4 Performance Analysis (2h)**

- **3.4.1 Sync latency (40 min):**
    - Method, table (min/max/mean/p95/stddev), histogram. Conclude vs NF-07 (≤ 2s).
- **3.4.2 File load time (25 min):**
    - Table (strokes | file size | load time), line chart. Conclude vs NF-08 (≤ 1s).
- **3.4.3 Scalability (25 min):**
    - Table (clients | latency | CPU | RAM). Conclude vs NF-09 (≥ 3 users).
- **3.4.4 FPS (20 min):**
    - Table (strokes | avg FPS | min FPS). Conclude vs NF-10 (≥ 30 FPS).
- **3.4.5 Chunking (10 min):**
    - Brief table, small vs chunked comparison.

---

#### **Day 8 — Sections 3.5, 3.6, 3.7 (2h)**

- **3.5.1 CRDT convergence (30 min):**
    - Describe concurrent-draw and clear-during-draw experiments. Evidence (matching stroke lists). Conclude vs NF-06.
- **3.5.2 Offline continuity (20 min):**
    - Experiment or document limitation. Conclude vs NF-05.
- **3.6 Usability (30 min):**
    - Qualitative NF-01..NF-04 evaluation with screenshots.
- **3.7 NF fulfillment summary table (40 min):**
    - Capstone table: NF-ID | requirement | measured value | pass/fail (NF-01 to NF-14). Each row with concrete evidence.

---

#### **Day 9 — Section 3.8, Polish & Compile (2h)**

- **3.8 Limitations (30 min):**
    - Eraser paints white (no background restore), document growth from eraser, no undo/redo, fixed 800×600 canvas, any bugs found. Briefly discuss mitigations.
- **Polish full chapter (60 min):**
    - Consistency, cross-references, table formatting. No design/architecture content — only measured results.
- **Compile & fix (30 min):**
    - `latexmk thesis-en.tex`, fix errors/missing images/broken refs, verify TOC and lists.

---

## Key Decisions

- All performance measurements use `--release` builds.
- Sync latency = end-to-end (stroke commit on A → render on B), not just network RTT.
- If NF-05 (offline continuity) isn't supported, document as limitation.
- Charts: use `pgfplots` in LaTeX or Python `matplotlib` for consistency.

---

## Verification Checklist

- `cargo test` green for all tests (old + new).
- All 14 NF requirements have concrete measured values in 3.7 summary table.
- All 10 MAN scenarios have pass/fail + screenshots.
- `latexmk thesis-en.tex` compiles without errors.
- Chapter 3 contains only measurements, tables, and conclusions — no design descriptions.
