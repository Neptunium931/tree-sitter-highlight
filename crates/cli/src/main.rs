use anstyle::{AnsiColor, Color, Style};
use anyhow::{Result, anyhow};
use clap::{Args, Command, FromArgMatches as _, Subcommand, ValueEnum, crate_authors};
use clap_complete::generate;
use log::{error, info, warn};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use tree_sitter::ffi;
use tree_sitter_cli::{
    highlight::{self, HighlightOptions},
    input::{CliInput, get_input, get_tmp_source_file},
    logger, util,
};
use tree_sitter_config::Config;
use tree_sitter_loader::{self as loader};

const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_SHA: Option<&'static str> = option_env!("BUILD_SHA");

#[derive(Subcommand)]
#[command(about="Generates and tests parsers", author=crate_authors!("\n"), styles=get_styles())]
enum Commands {
    /// Highlight a file
    Highlight(Highlight),
    /// Print info about all known language parsers
    DumpLanguages(DumpLanguages),
    /// Generate shell completions
    Complete(Complete),
}

#[derive(ValueEnum, Clone)]
pub enum Encoding {
    Utf8 = 0,
    Utf16LE = 1,
    Utf16BE = 2,
}

#[derive(Args)]
#[command(alias = "hi")]
struct Highlight {
    /// Generate highlighting as an HTML document
    #[arg(long, short = 'H', conflicts_with = "latex")]
    pub html: bool,
    /// When generating HTML, use css classes rather than inline styles
    #[arg(long)]
    pub css_classes: bool,
    /// Generate highlighting as an LaTeX document
    #[arg(long, short = 'L', conflicts_with = "html")]
    pub latex: bool,
    /// Check that highlighting captures conform strictly to standards
    #[arg(long)]
    pub check: bool,
    /// The path to a file with captures
    #[arg(long)]
    pub captures_path: Option<PathBuf>,
    /// The paths to files with queries
    #[arg(long, num_args = 1..)]
    pub query_paths: Option<Vec<PathBuf>>,
    /// Select a language by the scope instead of a file extension
    #[arg(long)]
    pub scope: Option<String>,
    /// Measure execution time
    #[arg(long, short)]
    pub time: bool,
    /// Suppress main output
    #[arg(long, short)]
    pub quiet: bool,
    /// The path to a file with paths to source file(s)
    #[arg(long = "paths")]
    pub paths_file: Option<PathBuf>,
    /// The source file(s) to use
    #[arg(num_args = 1..)]
    pub paths: Option<Vec<PathBuf>>,
    /// The path to the tree-sitter grammar directory, implies --rebuild
    #[arg(long, short = 'p', conflicts_with = "rebuild")]
    pub grammar_path: Option<PathBuf>,
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    /// Highlight the contents of a specific test
    #[arg(long, short = 'n')]
    #[clap(conflicts_with = "paths", conflicts_with = "paths_file")]
    pub test_number: Option<u32>,
    /// Force rebuild the parser
    #[arg(short, long)]
    pub rebuild: bool,
    /// The encoding of the input files
    #[arg(long)]
    pub encoding: Option<Encoding>,
}

#[derive(Args)]
#[command(alias = "langs")]
struct DumpLanguages {
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
}

#[derive(Args)]
#[command(alias = "comp")]
struct Complete {
    /// The shell to generate completions for
    #[arg(long, short, value_enum)]
    pub shell: Shell,
}

#[derive(ValueEnum, Clone)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Nushell,
}

impl Highlight {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let theme_config: tree_sitter_cli::highlight::ThemeConfig = config.get()?;
        loader.configure_highlights(&theme_config.theme.highlight_names);
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;
        loader.force_rebuild(self.rebuild || self.grammar_path.is_some());
        let languages = loader.languages_at_path(current_dir)?;

        let cancellation_flag = util::cancel_on_signal();

        let (mut language, mut language_configuration) = (None, None);
        if let Some(scope) = self.scope.as_deref() {
            if let Some((lang, lang_config)) = loader.language_configuration_for_scope(scope)? {
                language = Some(lang);
                language_configuration = Some(lang_config);
            }
            if language.is_none() {
                return Err(anyhow!("Unknown scope '{scope}'"));
            }
        }

        let encoding = self.encoding.map(|e| match e {
            Encoding::Utf8 => ffi::TSInputEncodingUTF8,
            Encoding::Utf16LE => ffi::TSInputEncodingUTF16LE,
            Encoding::Utf16BE => ffi::TSInputEncodingUTF16BE,
        });

        let options = HighlightOptions {
            theme: theme_config.theme,
            check: self.check,
            captures_path: self.captures_path,
            inline_styles: !self.css_classes,
            html: self.html,
            latex: self.latex,
            quiet: self.quiet,
            print_time: self.time,
            cancellation_flag: cancellation_flag.clone(),
            encoding,
        };

        let input = get_input(
            self.paths_file.as_deref(),
            self.paths,
            self.test_number,
            &cancellation_flag,
        )?;
        match input {
            CliInput::Paths(paths) => {
                let print_name = paths.len() > 1;
                for path in paths {
                    let (language, language_config) =
                        match (language.clone(), language_configuration) {
                            (Some(l), Some(lc)) => (l, lc),
                            _ => {
                                if let Some((lang, lang_config)) =
                                    loader.language_configuration_for_file_name(&path)?
                                {
                                    (lang, lang_config)
                                } else {
                                    warn!(
                                        "{}",
                                        util::lang_not_found_for_path(&path, &loader_config)
                                    );
                                    continue;
                                }
                            }
                        };

                    if let Some(highlight_config) =
                        language_config.highlight_config(language, self.query_paths.as_deref())?
                    {
                        highlight::highlight(
                            &loader,
                            &path,
                            &path.display().to_string(),
                            highlight_config,
                            print_name,
                            &options,
                        )?;
                    } else {
                        warn!(
                            "No syntax highlighting config found for path {}",
                            path.display()
                        );
                    }
                }
            }

            CliInput::Test {
                name,
                contents,
                languages: language_names,
            } => {
                let path = get_tmp_source_file(&contents)?;

                let language = languages
                    .iter()
                    .find(|(_, n)| language_names.contains(&Box::from(n.as_str())))
                    .or_else(|| languages.first())
                    .map(|(l, _)| l.clone())
                    .ok_or_else(|| anyhow!("No language found in current path"))?;
                let language_config = loader
                    .get_language_configuration_in_current_path()
                    .ok_or_else(|| anyhow!("No language configuration found in current path"))?;

                if let Some(highlight_config) =
                    language_config.highlight_config(language, self.query_paths.as_deref())?
                {
                    highlight::highlight(&loader, &path, &name, highlight_config, false, &options)?;
                } else {
                    warn!("No syntax highlighting config found for test {name}");
                }
                fs::remove_file(path)?;
            }

            CliInput::Stdin(contents) => {
                // Place user input and highlight output on separate lines
                println!();

                let path = get_tmp_source_file(&contents)?;

                let (language, language_config) =
                    if let (Some(l), Some(lc)) = (language, language_configuration) {
                        (l, lc)
                    } else {
                        let language = languages
                            .first()
                            .map(|(l, _)| l.clone())
                            .ok_or_else(|| anyhow!("No language found in current path"))?;
                        let language_configuration = loader
                            .get_language_configuration_in_current_path()
                            .ok_or_else(|| {
                                anyhow!("No language configuration found in current path")
                            })?;
                        (language, language_configuration)
                    };

                if let Some(highlight_config) =
                    language_config.highlight_config(language, self.query_paths.as_deref())?
                {
                    highlight::highlight(
                        &loader,
                        &path,
                        "stdin",
                        highlight_config,
                        false,
                        &options,
                    )?;
                } else {
                    warn!(
                        "No syntax highlighting config found for path {}",
                        current_dir.display()
                    );
                }
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

impl DumpLanguages {
    fn run(self, mut loader: loader::Loader) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;
        for (configuration, language_path) in loader.get_all_language_configurations() {
            info!(
                concat!(
                    "name: {}\n",
                    "scope: {}\n",
                    "parser: {:?}\n",
                    "highlights: {:?}\n",
                    "file_types: {:?}\n",
                    "content_regex: {:?}\n",
                    "injection_regex: {:?}\n",
                ),
                configuration.language_name,
                configuration.scope.as_ref().unwrap_or(&String::new()),
                language_path,
                configuration.highlights_filenames,
                configuration.file_types,
                configuration.content_regex,
                configuration.injection_regex,
            );
        }
        Ok(())
    }
}

impl Complete {
    fn run(self, cli: &mut Command) {
        let name = cli.get_name().to_string();
        let mut stdout = std::io::stdout();

        match self.shell {
            Shell::Bash => generate(clap_complete::shells::Bash, cli, &name, &mut stdout),
            Shell::Elvish => generate(clap_complete::shells::Elvish, cli, &name, &mut stdout),
            Shell::Fish => generate(clap_complete::shells::Fish, cli, &name, &mut stdout),
            Shell::PowerShell => {
                generate(clap_complete::shells::PowerShell, cli, &name, &mut stdout);
            }
            Shell::Zsh => generate(clap_complete::shells::Zsh, cli, &name, &mut stdout),
            Shell::Nushell => generate(clap_complete_nushell::Nushell, cli, &name, &mut stdout),
        }
    }
}

fn main() {
    let result = run();
    if let Err(err) = &result {
        // Ignore BrokenPipe errors
        if let Some(error) = err.downcast_ref::<std::io::Error>()
            && error.kind() == std::io::ErrorKind::BrokenPipe
        {
            return;
        }
        if !err.to_string().is_empty() {
            error!("{err:?}");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    logger::init();

    let version = BUILD_SHA.map_or_else(
        || BUILD_VERSION.to_string(),
        |build_sha| format!("{BUILD_VERSION} ({build_sha})"),
    );

    let cli = Command::new("tree-sitter-highlight")
        .help_template(concat!(
            "\n",
            "{before-help}{name} {version}\n",
            "{author-with-newline}{about-with-newline}\n",
            "{usage-heading} {usage}\n",
            "\n",
            "{all-args}{after-help}\n",
            "\n"
        ))
        .version(version)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .disable_help_subcommand(true)
        .disable_colored_help(false);
    let mut cli = Commands::augment_subcommands(cli);

    let command = Commands::from_arg_matches(&cli.clone().get_matches())?;

    let current_dir = match &command {
        Commands::Highlight(_) | Commands::DumpLanguages(_) | Commands::Complete(_) => &None,
    }
    .as_ref()
    .map_or_else(|| env::current_dir().unwrap(), std::clone::Clone::clone);

    let loader = loader::Loader::new()?;

    match command {
        Commands::Highlight(highlight_options) => highlight_options.run(loader, &current_dir)?,
        Commands::DumpLanguages(dump_options) => dump_options.run(loader)?,
        Commands::Complete(complete_options) => complete_options.run(&mut cli),
    }

    Ok(())
}

#[must_use]
const fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .header(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
        .invalid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .valid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::White))))
}
