use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use nitro_core::{DaemonCommand, PowerState, Profile};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table},
    Terminal,
};
use std::io;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::time;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channels
    let (tx_state, mut rx_state) = mpsc::channel::<PowerState>(10);
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<DaemonCommand>(10);

    // Spawn Network Task
    tokio::spawn(async move {
        loop {
            if let Ok(stream) = UnixStream::connect("/tmp/nitro.sock").await {
                let (reader, mut writer) = stream.into_split();
                let mut lines = BufReader::new(reader).lines();

                // Reader Task
                let tx_state = tx_state.clone();
                let mut reader_handle = tokio::spawn(async move {
                    while let Ok(Some(line)) = lines.next_line().await {
                        if let Ok(state) = serde_json::from_str::<PowerState>(&line) {
                            if tx_state.send(state).await.is_err() {
                                break;
                            }
                        }
                    }
                });

                // Writer Loop (in current task)
                loop {
                    tokio::select! {
                        _ = &mut reader_handle => {
                            // Reader died (connection closed)
                            break;
                        }
                        cmd = rx_cmd.recv() => {
                            if let Some(cmd) = cmd {
                                if let Ok(json) = serde_json::to_string(&cmd) {
                                    if writer.write_all(format!("{}\n", json).as_bytes()).await.is_err() {
                                        break;
                                    }
                                }
                            } else {
                                // Channel closed, exit app
                                return;
                            }
                        }
                    }
                }
            }
            // Retry connection every second if failed or disconnected
            time::sleep(Duration::from_secs(1)).await;
        }
    });

    // App Loop
    let mut state = PowerState {
        battery_watts: 0.0,
        cpu_watts: 0.0,
        battery_percent: 0,
        cpu_load: 0.0,
        profile: Profile::Eco,
        wifi_on: false,
        bluetooth_on: false,
        is_plugged_in: false,
    };

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = time::Instant::now();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(20), // Gauge
                        Constraint::Percentage(40), // Watts
                        Constraint::Percentage(40), // Details
                    ]
                    .as_ref(),
                )
                .split(f.size());

            // 1. Battery Gauge
            let gauge_color = if state.battery_percent > 50 {
                Color::Green
            } else if state.battery_percent > 20 {
                Color::Yellow
            } else {
                Color::Red
            };

            let gauge = Gauge::default()
                .block(Block::default().title("Battery").borders(Borders::ALL))
                .gauge_style(Style::default().fg(gauge_color))
                .percent(state.battery_percent as u16);
            f.render_widget(gauge, chunks[0]);

            // 2. Wattage
            // 2. Wattage
            let watt_text = if state.is_plugged_in {
                vec![Span::styled(
                    "CHARGING",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::ITALIC),
                )]
            } else {
                vec![
                    Span::styled(
                        format!("TOTAL: {:.1} W", state.battery_watts),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("\n"),
                    Span::styled(
                        format!("CPU:   {:.1} W", state.cpu_watts),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]
            };

            let paragraph = Paragraph::new(ratatui::text::Text::from(
                watt_text
                    .into_iter()
                    .map(ratatui::text::Line::from)
                    .collect::<Vec<_>>(),
            ))
            .block(Block::default().title("Power Draw").borders(Borders::ALL))
            .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(paragraph, chunks[1]);

            // 3. Details Table
            let rows = vec![
                Row::new(vec![
                    Cell::from("Profile"),
                    Cell::from(format!("{:?}", state.profile)),
                ]),
                Row::new(vec![
                    Cell::from("CPU Load"),
                    Cell::from(format!("{:.2}", state.cpu_load)),
                ]),
                Row::new(vec![
                    Cell::from("WiFi"),
                    Cell::from(if state.wifi_on { "ON" } else { "OFF" }),
                ]),
                Row::new(vec![
                    Cell::from("Bluetooth"),
                    Cell::from(if state.bluetooth_on { "ON" } else { "OFF" }),
                ]),
            ];

            let table = Table::new(
                rows,
                [Constraint::Percentage(50), Constraint::Percentage(50)],
            )
            .block(Block::default().title("Details").borders(Borders::ALL));
            f.render_widget(table, chunks[2]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('m') => {
                        let _ = tx_cmd.send(DaemonCommand::SetProfile(Profile::Monk)).await;
                    }
                    KeyCode::Char('e') => {
                        let _ = tx_cmd.send(DaemonCommand::SetProfile(Profile::Eco)).await;
                    }
                    KeyCode::Char('p') => {
                        let _ = tx_cmd.send(DaemonCommand::SetProfile(Profile::Pro)).await;
                    }
                    _ => {}
                }
            }
        }

        // Check for new state
        while let Ok(new_state) = rx_state.try_recv() {
            state = new_state;
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = time::Instant::now();
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
