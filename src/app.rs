use crate::error::Error;
use crate::util;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Selected {
    Feeds,
    Entries,
    Entry(crate::rss::Entry),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Editing,
    Normal,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReadMode {
    ShowAll,
    ShowUnread,
}

#[derive(Debug)]
pub(crate) struct App<'app> {
    pub title: &'app str,
    pub database_path: PathBuf,
    pub conn: rusqlite::Connection,
    pub enhanced_graphics: bool,
    pub should_quit: bool,
    pub progress: f64,
    pub error_flash: Option<Error>,
    pub feed_titles: util::StatefulList<(i64, String)>,
    pub entries: util::StatefulList<crate::rss::Entry>,
    pub selected: Selected,
    pub scroll: u16,
    pub current_entry: Option<crate::rss::Entry>,
    pub current_entry_text: Vec<tui::widgets::Text<'app>>,
    pub current_feed: Option<crate::rss::Feed>,
    pub input: String,
    pub mode: Mode,
    pub read_mode: ReadMode,
    pub entry_selection_position: usize,
}

impl<'app> App<'app> {
    pub(crate) fn new(
        title: &'app str,
        database_path: PathBuf,
        enhanced_graphics: bool,
    ) -> Result<App<'app>, Error> {
        let conn = rusqlite::Connection::open(&database_path)?;
        crate::rss::initialize_db(&conn)?;
        let initial_feed_titles = vec![].into();
        let selected = Selected::Feeds;
        let initial_current_feed = None;
        let initial_entries = vec![].into();

        let mut app = App {
            title,
            database_path,
            conn,
            enhanced_graphics,
            progress: 0.0,
            should_quit: false,
            error_flash: None,
            feed_titles: initial_feed_titles,
            entries: initial_entries,
            selected,
            scroll: 0,
            current_entry: None,
            current_entry_text: vec![],
            current_feed: initial_current_feed,
            input: String::new(),
            mode: Mode::Normal,
            read_mode: ReadMode::ShowUnread,
            entry_selection_position: 0,
        };

        app.update_feed_titles()?;
        app.update_current_feed_and_entries()?;

        Ok(app)
    }

    pub fn update_feed_titles(&mut self) -> Result<(), Error> {
        let feed_titles = crate::rss::get_feed_titles(&self.conn)?.into();
        self.feed_titles = feed_titles;
        Ok(())
    }

    pub fn update_current_feed_and_entries(&mut self) -> Result<(), Error> {
        self.update_current_feed()?;
        self.update_current_entries()?;
        Ok(())
    }

    fn update_current_feed(&mut self) -> Result<(), Error> {
        let current_feed = if self.feed_titles.items.is_empty() {
            None
        } else {
            let selected_idx = match self.feed_titles.state.selected() {
                Some(idx) => idx,
                None => {
                    self.feed_titles.state.select(Some(0));
                    0
                }
            };
            let feed_id = self.feed_titles.items[selected_idx].0;
            Some(crate::rss::get_feed(&self.conn, feed_id)?)
        };

        self.current_feed = current_feed;

        Ok(())
    }

    fn update_current_entries(&mut self) -> Result<(), Error> {
        let entries = if let Some(feed) = &self.current_feed {
            crate::rss::get_entries(&self.conn, &self.read_mode, feed.id)?
                .into_iter()
                .collect::<Vec<_>>()
                .into()
        } else {
            vec![].into()
        };

        self.entries = entries;
        if self.entry_selection_position < self.entries.items.len() {
            self.entries
                .state
                .select(Some(self.entry_selection_position))
        } else {
            self.entries
                .state
                .select(Some(self.entries.items.len() - 1))
        }
        Ok(())
    }

    pub async fn select_feeds(&mut self) {
        self.selected = Selected::Feeds;
    }

    pub async fn subscribe_to_feed(&mut self) -> Result<(), Error> {
        let _feed_id = crate::rss::subscribe_to_feed(&self.conn, &self.input).await?;
        let feed_titles = crate::rss::get_feed_titles(&self.conn)?.into();
        self.feed_titles = feed_titles;
        Ok(())
    }

    fn get_selected_entry(&self) -> Option<Result<crate::rss::Entry, Error>> {
        if let Some(selected_idx) = self.entries.state.selected() {
            if let Some(entry_id) = self.entries.items.get(selected_idx).map(|item| item.id) {
                Some(crate::rss::get_entry(&self.conn, entry_id))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn on_up(&mut self) -> Result<(), Error> {
        match self.selected {
            Selected::Feeds => {
                self.feed_titles.previous();
                self.update_current_feed_and_entries()?;
            }
            Selected::Entries => {
                if !self.entries.items.is_empty() {
                    self.entries.previous();
                    self.entry_selection_position = self.entries.state.selected().unwrap();
                    if let Some(entry) = self.get_selected_entry() {
                        let entry = entry?;
                        self.current_entry = Some(entry);
                    }
                }
            }
            Selected::Entry(_) => {
                if let Some(n) = self.scroll.checked_sub(1) {
                    self.scroll = n
                };
            }
        }

        Ok(())
    }

    pub fn on_down(&mut self) -> Result<(), Error> {
        match self.selected {
            Selected::Feeds => {
                self.feed_titles.next();
                self.update_current_feed_and_entries()?;
            }
            Selected::Entries => {
                if !self.entries.items.is_empty() {
                    self.entries.next();
                    self.entry_selection_position = self.entries.state.selected().unwrap();
                    if let Some(entry) = self.get_selected_entry() {
                        let entry = entry?;
                        self.current_entry = Some(entry);
                    }
                }
            }
            Selected::Entry(_) => {
                if let Some(n) = self.scroll.checked_add(1) {
                    self.scroll = n
                };
            }
        }

        Ok(())
    }

    pub fn on_right(&mut self) -> Result<(), Error> {
        match self.selected {
            Selected::Feeds => {
                if !self.entries.items.is_empty() {
                    self.selected = Selected::Entries;
                    self.entries.state.select(Some(0));
                    if let Some(entry) = self.get_selected_entry() {
                        let entry = entry?;
                        self.current_entry = Some(entry);
                    }
                }
                Ok(())
            }
            Selected::Entries => self.on_enter(),
            Selected::Entry(_) => Ok(()),
        }
    }

    pub fn on_left(&mut self) {
        match self.selected {
            Selected::Feeds => (),
            Selected::Entries => self.selected = Selected::Feeds,
            Selected::Entry(_) => {
                self.scroll = 0;
                self.selected = {
                    self.current_entry_text = vec![];
                    Selected::Entries
                }
            }
        }
    }

    pub fn on_enter(&mut self) -> Result<(), Error> {
        match self.selected {
            Selected::Entries => {
                if !self.entries.items.is_empty() {
                    if let Some(entry) = &self.current_entry {
                        let empty_string = String::from("No content or description tag provided.");

                        // try content tag first,
                        // if there is not content tag,
                        // go to description tag,
                        // if no description tag,
                        // use empty string.
                        // TODO figure out what to actually do if there are neither
                        let entry_text = &entry
                            .content
                            .as_ref()
                            .or_else(|| entry.description.as_ref())
                            .or_else(|| Some(&empty_string));

                        // TODO make this width configurable
                        // TODO config should be in the database!
                        let text = html2text::from_read(entry_text.clone().unwrap().as_bytes(), 90);

                        let text = text
                            .split('\n')
                            .map(|line| {
                                tui::widgets::Text::raw({
                                    let mut owned = line.to_owned();
                                    owned.push_str("\n");
                                    owned
                                })
                            })
                            .collect::<Vec<_>>();

                        self.selected = Selected::Entry(entry.clone());
                        self.current_entry_text = text;
                    }
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn on_esc(&mut self) {
        match self.selected {
            Selected::Entry(_) => self.selected = Selected::Entries,
            Selected::Entries => (),
            Selected::Feeds => (),
        }
    }

    pub async fn on_refresh(&mut self) -> Result<(), Error> {
        let selected_idx = self.feed_titles.state.selected().unwrap();
        let feed_id = self.feed_titles.items[selected_idx].0;
        let _ = crate::rss::refresh_feed(&self.conn, feed_id).await?;
        self.update_current_feed_and_entries()?;
        Ok(())
    }

    pub async fn toggle_read(&mut self) -> Result<(), Error> {
        match &self.selected {
            Selected::Entry(entry) => {
                entry.toggle_read(&self.conn).await?;
                self.update_current_entries()?;
                if let Some(entry) = self.get_selected_entry() {
                    let entry = entry?;
                    self.current_entry = Some(entry);
                }
                self.selected = Selected::Entries;
                self.scroll = 0;
                // self.on_enter()?
            }
            Selected::Entries => {
                if let Some(entry) = &self.current_entry {
                    entry.toggle_read(&self.conn).await?;
                    self.update_current_entries()?;
                    if let Some(entry) = self.get_selected_entry() {
                        let entry = entry?;
                        self.current_entry = Some(entry);
                    }
                }
            }
            Selected::Feeds => (),
        }

        Ok(())
    }

    pub async fn toggle_read_mode(&mut self) -> Result<(), Error> {
        match (&self.read_mode, &self.selected) {
            (ReadMode::ShowAll, Selected::Feeds) | (ReadMode::ShowAll, Selected::Entries) => {
                self.read_mode = ReadMode::ShowUnread
            }
            // => self.read_mode = ReadMode::ShowUnread,
            (ReadMode::ShowUnread, Selected::Feeds) | (ReadMode::ShowUnread, Selected::Entries) => {
                self.read_mode = ReadMode::ShowAll
            }
            _ => (),
        }
        self.update_current_entries()?;

        if !self.entries.items.is_empty() {
            self.entries.state.select(Some(0));
        } else {
            self.entries.state.select(None);
        }

        if let Some(entry) = self.get_selected_entry() {
            let entry = entry?;
            self.current_entry = Some(entry);
        }

        Ok(())
    }

    pub async fn on_key(&mut self, c: char) -> Result<(), Error> {
        match c {
            'q' => {
                self.should_quit = true;
            }
            // vim-style movement
            'h' => self.on_left(),
            'j' => self.on_down()?,
            'k' => self.on_up()?,
            'l' => self.on_right().unwrap(),
            // controls
            'r' => match self.selected {
                Selected::Feeds => return self.on_refresh().await,
                _ => return self.toggle_read().await,
            },
            'a' => self.toggle_read_mode().await?,
            'e' | 'i' => {
                self.mode = Mode::Editing;
            }
            _ => (),
        }

        Ok(())
    }

    pub fn on_tick(&mut self) {
        // Update progress
        self.progress += 0.001;
        if self.progress > 1.0 {
            self.progress = 0.0;
        }
    }
}
