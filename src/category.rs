use crate::Result;

/// Category used when the user does not choose one.
pub const DEFAULT: &str = "Utility";

/// Main categories of the Desktop Menu Specification.
///
/// A desktop entry must contain at least one of them so that the application
/// appears in a standard menu section.
const MAIN: &[&str] = &[
    "AudioVideo",
    "Audio",
    "Video",
    "Development",
    "Education",
    "HealthFitness",
    "Game",
    "Graphics",
    "Network",
    "Office",
    "Science",
    "Settings",
    "System",
    "Utility",
];

/// Additional categories of the Desktop Menu Specification.
const ADDITIONAL: &[&str] = &[
    "Building",
    "Debugger",
    "IDE",
    "GUIDesigner",
    "Profiling",
    "RevisionControl",
    "Translation",
    "Calendar",
    "ContactManagement",
    "Database",
    "Dictionary",
    "Chart",
    "Email",
    "Finance",
    "FlowChart",
    "PDA",
    "ProjectManagement",
    "Presentation",
    "Spreadsheet",
    "WordProcessor",
    "2DGraphics",
    "VectorGraphics",
    "RasterGraphics",
    "3DGraphics",
    "Scanning",
    "OCR",
    "Photography",
    "Publishing",
    "Viewer",
    "TextTools",
    "DesktopSettings",
    "HardwareSettings",
    "Printing",
    "PackageManager",
    "Dialup",
    "InstantMessaging",
    "Chat",
    "IRCClient",
    "Feed",
    "FileTransfer",
    "HamRadio",
    "News",
    "P2P",
    "RemoteAccess",
    "Telephony",
    "TelephonyTools",
    "VideoConference",
    "WebBrowser",
    "WebDevelopment",
    "Midi",
    "Mixer",
    "Sequencer",
    "Tuner",
    "TV",
    "AudioVideoEditing",
    "Player",
    "Recorder",
    "DiscBurning",
    "ActionGame",
    "AdventureGame",
    "ArcadeGame",
    "BoardGame",
    "BlocksGame",
    "CardGame",
    "KidsGame",
    "LogicGame",
    "RolePlaying",
    "Shooter",
    "Simulation",
    "SportsGame",
    "StrategyGame",
    "LauncherStore",
    "GameTool",
    "Exercise",
    "Medication",
    "Mindfulness",
    "Nutrition",
    "CareProvider",
    "Sleep",
    "Art",
    "Construction",
    "Music",
    "Languages",
    "ArtificialIntelligence",
    "Astronomy",
    "Biology",
    "Chemistry",
    "ComputerScience",
    "DataVisualization",
    "Economy",
    "Electricity",
    "Geography",
    "Geology",
    "Geoscience",
    "History",
    "Humanities",
    "ImageProcessing",
    "Literature",
    "Maps",
    "Math",
    "NumericalAnalysis",
    "MedicalSoftware",
    "Physics",
    "Robotics",
    "Spirituality",
    "Sports",
    "ParallelComputing",
    "Amusement",
    "Archiving",
    "Compression",
    "Electronics",
    "Emulator",
    "Engineering",
    "FileTools",
    "FileManager",
    "TerminalEmulator",
    "Filesystem",
    "Monitor",
    "Security",
    "Accessibility",
    "Calculator",
    "Clock",
    "TextEditor",
    "Documentation",
    "Adult",
    "Core",
    "KDE",
    "COSMIC",
    "GNOME",
    "GTK",
    "LXQt",
    "XFCE",
    "DDE",
    "Qt",
    "Motif",
    "Java",
    "ConsoleOnly",
];

/// Reserved categories. The specification requires `OnlyShowIn` for them,
/// which TuiPWA does not write, so they are rejected instead of being emitted
/// into every desktop environment.
const RESERVED: &[&str] = &["Screensaver", "TrayIcon", "Applet", "Shell"];

/// Additional categories that the specification requires to be paired with
/// another category (`Audio` and `Video` must include `AudioVideo`).
const REQUIRED_WITH: &[(&str, &str)] = &[("Audio", "AudioVideo"), ("Video", "AudioVideo")];

pub fn is_main(name: &str) -> bool {
    MAIN.contains(&name)
}

pub fn is_additional(name: &str) -> bool {
    ADDITIONAL.contains(&name)
}

/// Validates, de-duplicates and orders a category list.
///
/// The order of the input is kept; only repeated names are dropped. Category
/// names are case sensitive and may only contain ASCII letters, digits and
/// `-`, which also keeps `;` and newlines out of the generated desktop entry.
pub fn normalize(names: &[String]) -> Result<Vec<String>> {
    let mut categories: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        validate_name(name)?;
        if !categories.contains(name) {
            categories.push(name.clone());
        }
    }
    if categories.is_empty() {
        return Err("カテゴリが指定されていません".into());
    }
    if !categories.iter().any(|name| is_main(name)) {
        return Err(
            "メインカテゴリを1つ以上指定してください（例: Utility, System, Development, Network）"
                .into(),
        );
    }
    for (name, required) in REQUIRED_WITH {
        if categories.iter().any(|c| c == name) && !categories.iter().any(|c| c == required) {
            return Err(format!("カテゴリ {name} には {required} も指定してください").into());
        }
    }
    Ok(categories)
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err("カテゴリ名が空です".into());
    }
    if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(format!(
            "カテゴリ名に使えない文字があります: {name}（英数字と - のみ使用できます）"
        )
        .into());
    }
    if RESERVED.contains(&name) {
        return Err(format!("予約カテゴリ {name} は OnlyShowIn が必要なため指定できません").into());
    }
    if name.starts_with("X-") {
        if name.len() == 2 {
            return Err(format!("独自カテゴリの名前がありません: {name}").into());
        }
        return Ok(());
    }
    if is_main(name) || is_additional(name) {
        return Ok(());
    }
    Err(format!("未知のカテゴリです: {name}（独自カテゴリは X- で始めてください）").into())
}

/// The value of the `Categories=` field, including the trailing `;`.
pub fn desktop_value(categories: &[String]) -> String {
    format!("{};", categories.join(";"))
}
