// Declare that we want to use the 'scraper' module defined in scraper.rs
mod scraper;

// Import tools from 'crossterm' to control the terminal (reading keys, alternate screens)
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

// Import UI widgets and layout tools from 'ratatui'
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Gauge, Paragraph},
    Frame, Terminal,
};

// Import our custom data types and functions from the scraper module
use scraper::{scrape_horoscope, HoroscopeData, ZodiacSign};

// Standard library imports for handling collections, errors, and timing
use std::{collections::HashMap, error::Error, io, time::Duration};

// Import Multi-Producer Single-Consumer channels from tokio for async communication
use tokio::sync::mpsc;

// This enum defines every type of event that our main application loop can respond to.
// By centralizing events, we ensure the UI thread is never blocked waiting for a specific task.
enum AppEvent {
    // A regular timer tick (useful for animations or forcing screen updates)
    Tick,
    // A user pressed a key on their keyboard
    Key(event::KeyEvent),
    // Background task successfully fetched data for a sign
    DataFetched(HoroscopeData),
    // Background task encountered an error
    FetchError(ZodiacSign, String),
}

// The 'App' struct holds all the state for our application.
// This includes which sign is currently focused, the raw data, and tracking loading progress.
struct App {
    focused_index: usize,
    signs: Vec<ZodiacSign>,
    data: HashMap<ZodiacSign, HoroscopeData>,
    errors: HashMap<ZodiacSign, String>,
    fetches_completed: usize,
}

impl App {
    // Constructor to initialize the App state
    fn new() -> App {
        App {
            focused_index: 0,
            signs: ZodiacSign::ALL.to_vec(),
            data: HashMap::new(),
            errors: HashMap::new(),
            fetches_completed: 0,
        }
    }

    // Move focus to the next sign (loops back to the beginning)
    fn next(&mut self) {
        self.focused_index = (self.focused_index + 1) % self.signs.len();
    }

    // Move focus to the previous sign (loops to the end)
    fn previous(&mut self) {
        if self.focused_index == 0 {
            self.focused_index = self.signs.len() - 1;
        } else {
            self.focused_index -= 1;
        }
    }

    // Check if the application is still waiting for background tasks to finish
    fn is_loading(&self) -> bool {
        self.fetches_completed < self.signs.len()
    }

    // Calculate the overall score by averaging all category percentages
    fn get_overall_score(data: &HoroscopeData) -> u64 {
        if data.scores.is_empty() {
            return 0;
        }
        
        let total: u64 = data
            .scores
            .iter()
            .map(|s| s.percent as u64)
            .sum();
            
        total / data.scores.len() as u64
    }
}

// The tokio::main macro sets up the async runtime automatically
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    
    // 1. Set up the terminal for the TUI
    // Raw mode gives us full control over user input, bypassing standard terminal line buffering
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    
    // Switch to an alternate screen so we don't overwrite the user's terminal history
    execute!(stdout, EnterAlternateScreen)?;
    
    // Initialize the ratatui Terminal instance
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Initialize application state and communication channels
    let mut app = App::new();
    
    // The channel allows background tasks (like reading keys or fetching data) 
    // to send messages safely back to the main UI thread.
    let (tx, mut rx) = mpsc::channel(128);

    // 3. Spawn a background task to constantly monitor keyboard input and emit timer ticks
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        loop {
            // Check if there is an input event ready (with a 100ms timeout)
            if event::poll(Duration::from_millis(100)).unwrap() {
                if let Event::Key(key) = event::read().unwrap() {
                    // Send the key press to the main loop
                    if tx_clone.send(AppEvent::Key(key)).await.is_err() {
                        break;
                    }
                }
            }
            
            // Send a tick to keep the UI updating smoothly
            if tx_clone.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // 4. Spawn 12 concurrent background tasks to fetch data for all signs simultaneously
    // This makes the application incredibly fast as it doesn't wait for one request to finish before starting the next.
    for &sign in &app.signs {
        let tx = tx.clone();
        tokio::spawn(async move {
            match scrape_horoscope(sign).await {
                Ok(data) => {
                    let _ = tx.send(AppEvent::DataFetched(data)).await;
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::FetchError(sign, format!("{:?}", e))).await;
                }
            }
        });
    }

    // 5. Start the main application loop
    let res = run_app(&mut terminal, &mut app, &mut rx).await;

    // 6. Gracefully shut down and restore the terminal to its normal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

// The core logic loop. It draws the UI and handles incoming events sequentially.
async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    rx: &mut mpsc::Receiver<AppEvent>,
) -> Result<(), io::Error>
where
    io::Error: From<<B as Backend>::Error>,
{
    loop {
        // Redraw the terminal screen using our custom `ui` function
        terminal.draw(|f| ui(f, app))?;

        // Wait for the next event from any background task
        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    // Only respond when a key is pressed down (ignore key release events)
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            // 'q' or Esc to quit
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            // Navigate right
                            KeyCode::Right | KeyCode::Char('l') => app.next(),
                            // Navigate left
                            KeyCode::Left | KeyCode::Char('h') => app.previous(),
                            // Ignore any other keys
                            _ => {}
                        }
                    }
                }
                AppEvent::DataFetched(data) => {
                    // Save the data and increment the loading counter
                    app.data.insert(data.sign, data);
                    app.fetches_completed += 1;
                }
                AppEvent::FetchError(sign, msg) => {
                    // Save the error and increment the loading counter
                    app.errors.insert(sign, msg);
                    app.fetches_completed += 1;
                }
                // Do nothing specific on ticks right now, but they force the loop to continue and redraw
                AppEvent::Tick => {}
            }
        }
    }
}

// The visual rendering function. It takes the current App state and paints it onto the terminal frame.
fn ui(f: &mut Frame, app: &mut App) {
    
    // --- 1. Define the Color Palette ---
    // Using a sophisticated, abstract dark mode color scheme
    let bg_color = Color::Rgb(18, 18, 20);
    let border_color = Color::Rgb(60, 60, 65);
    let highlight_fg = Color::Rgb(200, 160, 255);
    let text_color = Color::Rgb(220, 220, 225);
    let title_color = Color::Rgb(160, 180, 255);
    let gauge_bg = Color::Rgb(30, 30, 35);
    let gauge_fg = Color::Rgb(100, 200, 180);
    let bar_color = Color::Rgb(80, 140, 220);
    
    // Colors specifically for the BarChart selection and winning logic
    let focused_bar_color = Color::Rgb(220, 180, 50);
    let winner_bar_color = Color::Rgb(255, 215, 0);

    // --- 2. Define the Main Layout Structure ---
    // Split the screen vertically into two equal halves (50% each)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.area());

    // Paint the entire background color first
    f.render_widget(
        Block::default().style(Style::default().bg(bg_color)), 
        f.area()
    );

    // --- 3. Render the Top Half (Bar Chart for all 12 signs) ---
    
    // Create the border block for the top section
    let top_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(
            Span::styled(
                " Overall Scores (Best Horoscope Overall) ",
                Style::default()
                    .fg(title_color)
                    .add_modifier(Modifier::BOLD)
            )
        );

    // Show a loading screen if we are still fetching data concurrently
    if app.is_loading() {
        let loading_text = Paragraph::new(format!("Loading... {}/12", app.fetches_completed))
            .block(top_block)
            .style(Style::default().fg(text_color));
            
        f.render_widget(loading_text, main_layout[0]);
        return; // Don't try to draw the rest of the UI until everything is loaded
    }

    // Calculate the absolute highest score to determine the "Winner of the Day"
    let mut highest_score = 0;
    for data in app.data.values() {
        let score = App::get_overall_score(data);
        if score > highest_score {
            highest_score = score;
        }
    }

    // Construct the data bars for the BarChart
    let mut bars = Vec::new();
    for (i, sign) in app.signs.iter().enumerate() {
        
        // Use an abbreviation for the X-axis labels (e.g., "ARI" instead of "Aries")
        let name = sign.as_str()[0..3].to_uppercase();
        
        let score = if let Some(data) = app.data.get(sign) {
            App::get_overall_score(data)
        } else {
            0
        };

        // Determine the dynamic styling for this specific bar
        let mut bar_style = Style::default().fg(bar_color);
        
        // Highlight the winner in bold, bright gold
        if score == highest_score && highest_score > 0 {
            bar_style = Style::default()
                .fg(winner_bar_color)
                .add_modifier(Modifier::BOLD);
        }
        
        // Highlight the user's currently selected sign with a distinct color
        if i == app.focused_index {
            // Only overwrite the color if it's not the winner, so the winner stays shiny gold!
            if score != highest_score {
                bar_style = Style::default()
                    .fg(focused_bar_color)
                    .add_modifier(Modifier::BOLD);
            }
        }

        // Add the configured bar to our list
        bars.push(
            Bar::default()
                .label(Line::from(name))
                .value(score)
                .style(bar_style)
                .text_value(format!("{}%", score)),
        );
    }

    // Group the bars together
    let bar_group = BarGroup::default().bars(&bars);

    // Create and render the BarChart widget
    let barchart = BarChart::default()
        .block(top_block)
        .data(bar_group)
        .bar_width(6)
        .bar_gap(2)
        .max(100)
        .value_style(
            Style::default()
                .fg(bg_color)
                .bg(bar_color)
        );

    f.render_widget(barchart, main_layout[0]);

    // --- 4. Render the Bottom Half (Detailed View for the Focused Sign) ---
    
    let focused_sign = app.signs[app.focused_index];

    // Create the border block for the bottom section
    let bottom_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(
            Span::styled(
                format!(" Details: {} ", focused_sign),
                Style::default()
                    .fg(title_color)
                    .add_modifier(Modifier::BOLD)
            )
        );

    // Calculate the inner usable area inside the border
    let inner_area = bottom_block.inner(main_layout[1]);
    f.render_widget(bottom_block, main_layout[1]);

    // Check if there were any network or parsing errors for this specific sign
    if let Some(err) = app.errors.get(&focused_sign) {
        let error_text = Paragraph::new(format!("Error: {}", err))
            .style(Style::default().fg(Color::Red));
            
        f.render_widget(error_text, inner_area);
        
    } else if let Some(data) = app.data.get(&focused_sign) {
        
        // Create a layout for the title and the list of gauges
        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)].as_ref())
            .split(inner_area);

        let header_text = Paragraph::new(
            Span::styled(
                format!("Category Breakdown for {}", focused_sign),
                Style::default()
                    .fg(highlight_fg)
                    .add_modifier(Modifier::BOLD)
            )
        );
        
        f.render_widget(header_text, header_layout[0]);

        let category_count = data.scores.len();
        if category_count > 0 {
            
            // Dynamically build constraints based on the number of categories found.
            // For each category, we need 2 lines for the Gauge, and 1 line for spacing.
            let mut constraints = vec![];
            for _ in 0..category_count {
                constraints.push(Constraint::Length(2));
                constraints.push(Constraint::Length(1));
            }
            constraints.push(Constraint::Min(0));

            let gauges_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(header_layout[1]);

            // Draw a Gauge widget for each category score (Mood, Love, etc.)
            for (i, score) in data.scores.iter().enumerate() {
                let gauge = Gauge::default()
                    .block(
                        Block::default()
                            .title(
                                Span::styled(
                                    score.category.clone(),
                                    Style::default().fg(text_color)
                                )
                            )
                    )
                    .gauge_style(
                        Style::default()
                            .fg(gauge_fg)
                            .bg(gauge_bg)
                    )
                    .percent(score.percent);
                    
                // Render into the even-indexed chunks (0, 2, 4...) because odd chunks are spaces
                f.render_widget(gauge, gauges_layout[i * 2]);
            }
        } else {
            let empty_text = Paragraph::new("No scores available.")
                .style(Style::default().fg(text_color));
                
            f.render_widget(empty_text, header_layout[1]);
        }
    }
}
