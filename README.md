# Horograph-R v0.1.0

## overview
horograph is a raw terminal interface designed to scrape, aggregate, and visualize astrological datasets in real-time. it bypasses the bloated `DOM` of traditional horoscope domains and extracts the pure numeric metrics underlying the daily predictions. written entirely in rust, it leverages asynchronous i/o to fetch 12 disparate network endpoints concurrently without blocking the primary rendering thread. 

## architecture
the core loop relies on `tokio` for async runtime management and `mpsc` channels for cross-thread event passing. as the binary initializes, 12 silent `tokio` tasks are spawned. each task executes a `reqwest::get` call to `free-horoscope.com`, pulls the raw html, and feeds it into the `scraper` crate. the `DOM` is parsed via hardcoded css selectors (`.astro-section-glass`, `.chart-col`) to extract inline height percentages. these percentages are packed into a rust struct and fired across the `mpsc` channel back to the main thread.

the presentation layer is driven by `ratatui` and `crossterm` operating in raw mode. the terminal alternate screen is hijacked to draw a split-pane layout. the upper pane renders an aggregated `BarChart` of the overall average score for all 12 signs. the entity with the highest resonance is dynamically highlighted in gold as the victor. the lower pane utilizes `ratatui` `Gauge` widgets to render the absolute percentage breakdown of the focused sign.

## execution
ensure the rust compiler is present on your system. execute the binary directly via `cargo`.

```bash
cargo run
```

## controls
navigate the dataset laterally using the standard vim-style keybindings (`h` and `l`) or the `Left`/`Right` arrow keys. the internal `focused_index` will shift, dynamically redrawing the lower gauge pane to reflect the selected sign's granular metrics. terminate the process with `q` or `Esc`.

## stack
`ratatui`. `crossterm`. `tokio`. `reqwest`. `scraper`.
