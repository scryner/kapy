use std::collections::HashMap;

use anyhow::{anyhow, Result};
use indicatif::{MultiProgress, ProgressStyle, ProgressBar};

pub struct Progress<'a> {
    container: MultiProgress,
    panels: HashMap<&'a str, ProgressBar>,
}

pub enum PanelType<'a> {
    Bar(&'a str, u64),
    Message(&'a str),
}

pub enum Update {
    Position(u64, Option<String>),
    Incr(Option<String>),
}

impl<'a> Progress<'a> {
    pub fn new(panel_defs: Vec<PanelType<'a>>) -> Self {
        let bar_style = ProgressStyle::with_template("{spinner:.white} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos:>5}/{len:5}")
            .unwrap()
            .progress_chars("#>-");

        let message_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

        let container = MultiProgress::new();
        let mut panels = HashMap::new();

        let mut prev_pb: &ProgressBar;


        for (i, def) in panel_defs.iter().enumerate() {

            match *def {
                PanelType::Bar(key, len) => {
                    let pb = container.add(ProgressBar::new(len));
                    pb.set_style(bar_style.clone());
                    panels.insert(key, pb);
                }
                PanelType::Message(key) => {
                    let pb = container.add(ProgressBar::new(8));
                    pb.set_style(message_style.clone());
                    panels.insert(key, pb);
                }
            }
        }

        Progress {
            container,
            panels,
        }
    }

    pub fn update(&self, key: &str, u: Update) {
        if let Some(pb) = self.panels.get(key) {
            match u {
                Update::Position(pos, msg) => {
                    pb.set_position(pos);
                    if let Some(msg) = msg {
                        pb.set_message(msg);
                    }
                }
                Update::Incr(msg) => {
                    pb.inc(1);
                    if let Some(msg) = msg {
                        pb.set_message(msg);
                    }
                }
            }
        }
    }

    pub fn finish(&self, key: &str) {
        if let Some(pb) = self.panels.get(key) {
            pb.finish();
        }
    }

    pub fn finish_with_message(&self, key : &str, message: &'static str) {
        if let Some(pb) = self.panels.get(key) {
            pb.finish_with_message(message);
        }
    }

    fn clear(&self) -> Result<()> {
        match self.container.clear() {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("Failed to clear: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;
    use super::*;

    #[test]
    fn progress_bar() {
        let panels = vec![
            PanelType::Bar("bar", 100),
            PanelType::Message("msg")
        ];

        let p = Progress::new(panels);

        for i in 0..100 {
            p.update("bar", Update::Incr(None));
            if i % 5 == 0 {
                p.update("msg", Update::Incr(Some(format!("{} is progressing...", i))));
            }
            thread::sleep(Duration::from_millis(15));
        }

        p.finish_with_message("bar", "done");
        p.finish("msg");
        thread::sleep(Duration::from_secs(1));

        p.clear().unwrap();
    }
}