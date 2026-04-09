 
Planned testing sessions and thesis chapter writing
Explore: Explore existing tests and code
Explore: Explore thesis structure
Reviewed Chapter 3 outline and full thesis content
Now let me read the Chapter 3 outline and the non-functional requirements table to align the testing plan precisely.

Reviewed thesis-en.tex and checked lines 1540-1713 and 675-725
Now let me check the existing tests and understand what instrumentation exists in the code.

Reviewed automerge_backend.rs and TESTING_MANUAL.md
I have a complete picture of your codebase, existing tests, non-functional requirements (NF-01 through NF-14), and the Chapter 3 outline already in your thesis. Here's the plan:

Created memory file plan.md

Plan: Testing & Writing "Analysis of the Solution"
Conduct all tests needed for the thesis, collect measured data, then write Chapter 3 using those results. 9 days, 2 hours each — 5 days testing, 4 days writing.

PART 1: Conducting Tests (Days 1–5)
Day 1 — Setup & Unit Tests (2h)

Document testing environment (30 min) — Record hardware specs (CPU, RAM, OS), run iperf3/ping between machines, note LiveKit Docker version, rustc --version. Screenshot everything.
Extend unit tests (60 min) — You currently have 5 tests in automerge_backend.rs. Add:
Concurrent draw from two peers (both draw, verify both strokes survive after sync) → NF-06
Clear + concurrent draw (Peer A clears, Peer B draws → verify add-wins) → NF-06
Load corrupted/invalid bytes → verify graceful error
3-peer sync chain (A→B→C) → multi-peer convergence
Run all tests, capture output (30 min) — cargo test 2>&1 | tee test_output.txt, screenshot terminal
Day 2 — Performance Instrumentation (2h)

Add timing code (90 min):
Benchmark test/binary for .crdt load time: programmatically generate files with 100/500/1000/5000 strokes, measure backend.load() with std::time::Instant → NF-08
FPS logging in update() (use egui's frame time or manual Instant wrapping) → NF-10
Timestamp logging at generate_sync_message (send) and receive_sync_message (receive) → NF-07
Verify it compiles and runs (30 min) — cargo build --release, quick smoke test
Day 3 — Performance Testing: Latency, File Load, FPS (2h)

Sync latency — NF-07 (45 min) — 2 clients, same room. Draw stroke on A, measure time to appear on B. 30–50 trials. Record min/max/mean/p95/stddev in CSV.
File load time — NF-08 (30 min) — Run benchmarks from Day 2. Record: stroke count | file size (KB) | load time (ms). 5 trials per size.
Rendering FPS — NF-10 (45 min) — Release build. Measure FPS with 0, 100, 500, 1000 existing strokes. Record avg/min FPS.
Day 4 — Scalability, Chunking, Reliability (2h)

Multi-user scalability — NF-09 (45 min) — Connect 2/3/4/5 clients. Each draws for 30s. Record: num clients | avg latency | max latency | CPU % | RAM (MB).
Chunking overhead (30 min) — Send payloads of ~1KB/10KB/50KB/200KB (strokes with many points). Measure delivery time. Verify chunking activates above 14KB.
CRDT convergence — NF-06 (30 min) — Two clients draw simultaneously for 60s. Save both .crdt files, compare stroke lists (must be identical). Repeat with clear-during-draw → verify new strokes survive.
Offline continuity — NF-05 (15 min) — Draw while disconnected (kill LiveKit Docker). Reconnect. Check if offline strokes sync. If not: document as limitation.
Day 5 — Manual Acceptance Tests & Usability (2h)

Execute MAN-01 through MAN-10 (75 min) — Follow TESTING_MANUAL.md step by step. Record pass/fail + screenshot for each.
Usability assessment — NF-01 to NF-04 (30 min) — Screenshots of: startup UI, connection status indicator, remote cursors, English interface. Optionally have 1–2 people try it for 10 min.
Organize all data (15 min) — Put CSVs, screenshots, notes into DYPLOM/2. thesis/images/ or a test_results/ folder, named by section.
PART 2: Writing Chapter 3 (Days 6–9)
Day 6 — Sections 3.1, 3.2, 3.3 (2h)

3.1 Testing environment (30 min) — Hardware, network, LiveKit config, Rust toolchain. Small environment summary table.
3.2 Unit testing (45 min) — Results table (test name | what it verifies | pass/fail). Include cargo test screenshot. Discuss coverage.
3.3 Manual acceptance testing (45 min) — Results table for MAN-01..MAN-10. 3–4 representative screenshots.
Day 7 — Section 3.4 Performance Analysis (2h)

3.4.1 Sync latency (40 min) — Describe method, present table (min/max/mean/p95/stddev), include histogram. Conclude vs NF-07 (≤ 2s).
3.4.2 File load time (25 min) — Table (strokes | file size | load time), line chart. Conclude vs NF-08 (≤ 1s).
3.4.3 Scalability (25 min) — Table (clients | latency | CPU | RAM). Conclude vs NF-09 (≥ 3 users).
3.4.4 FPS (20 min) — Table (strokes | avg FPS | min FPS). Conclude vs NF-10 (≥ 30 FPS).
3.4.5 Chunking (10 min) — Brief table, small vs chunked comparison.
Day 8 — Sections 3.5, 3.6, 3.7 (2h)

3.5.1 CRDT convergence (30 min) — Describe concurrent-draw and clear-during-draw experiments. Evidence (matching stroke lists). Conclude vs NF-06.
3.5.2 Offline continuity (20 min) — Experiment or document limitation. Conclude vs NF-05.
3.6 Usability (30 min) — Qualitative NF-01..NF-04 evaluation with screenshots.
3.7 NF fulfillment summary table (40 min) — The capstone table: NF-ID | requirement | measured value | pass/fail — one row per NF-01 through NF-14. Every row needs a concrete number or evidence, not just "yes."
Day 9 — Section 3.8, Polish & Compile (2h)

3.8 Limitations (30 min) — Eraser paints white (no background restore), document growth from eraser, no undo/redo, fixed 800×600 canvas, any bugs found during testing. Briefly discuss potential mitigations.
Polish full chapter (60 min) — Consistency of tense/terminology, \label/\ref cross-references, table formatting. Ensure zero design/architecture content — only measured results (per the slide's "Never Include" rules).
Compile & fix (30 min) — latexmk thesis-en.tex, fix errors/missing images/broken refs, verify TOC and list of figures/tables update.
Key Decisions
All performance measurements use --release builds (debug is not representative)
Sync latency = end-to-end (stroke commit on A → render on B), not just network RTT
If NF-05 (offline continuity) isn't supported, document it honestly as a limitation
Charts: recommend pgfplots in LaTeX or Python matplotlib for consistency
Verification Checklist
cargo test green for all old + new tests
All 14 NF requirements have a concrete measured value in the 3.7 summary table
All 10 MAN scenarios have pass/fail + screenshots
latexmk thesis-en.tex compiles without errors
Chapter 3 contains only measurements, tables, and conclusions — no design descriptions