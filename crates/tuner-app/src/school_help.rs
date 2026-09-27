use eframe::egui;

use crate::CommandDescriptor;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SchoolHelpCopy<'a> {
    pub title: &'a str,
    pub what_it_does: &'a str,
    pub when_to_use: Option<&'a str>,
    pub keep_in_mind: Option<&'a str>,
}

const HELP_WINDOW_ID: &str = "school-me-help-window";
const HELP_STATE_ID: &str = "school-me-help-window-state";

#[derive(Clone, Debug, Default)]
struct OwnedSchoolHelp {
    title: String,
    what_it_does: String,
    when_to_use: Option<String>,
    keep_in_mind: Option<String>,
}

impl From<&SchoolHelpCopy<'_>> for OwnedSchoolHelp {
    fn from(help: &SchoolHelpCopy<'_>) -> Self {
        Self {
            title: help.title.to_owned(),
            what_it_does: help.what_it_does.to_owned(),
            when_to_use: help.when_to_use.map(str::to_owned),
            keep_in_mind: help.keep_in_mind.map(str::to_owned),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SchoolHelpWindowState {
    candidate: Option<OwnedSchoolHelp>,
    current: Option<OwnedSchoolHelp>,
}

macro_rules! school_help_catalog {
    ($( $id:ident => ($title:literal, $what:literal, $when:literal, $keep:expr); )+) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[repr(usize)]
        pub(crate) enum SchoolHelpId {
            $( $id, )+
        }

        #[allow(dead_code)]
        pub(crate) const ALL_CUSTOM_HELP_IDS: &[SchoolHelpId] = &[
            $( SchoolHelpId::$id, )+
        ];

        const CUSTOM_HELP: &[SchoolHelpCopy<'static>] = &[
            $( SchoolHelpCopy {
                title: $title,
                what_it_does: $what,
                when_to_use: Some($when),
                keep_in_mind: $keep,
            }, )+
        ];

        pub(crate) fn custom_help(id: SchoolHelpId) -> &'static SchoolHelpCopy<'static> {
            &CUSTOM_HELP[id as usize]
        }
    };
}

school_help_catalog! {
    BrowserFilter => (
        "Filter the parameter list",
        "Type part of a map name, ID, or category to hide things that do not match.",
        "Use this when the list is long and you know a word from the map name.",
        Some("Filtering hides list entries; it does not delete maps."));
    BrowserParameter => (
        "Open a map",
        "Select a map row to open its table in the workspace.",
        "Open a map when you want to inspect its values, axes, or graph.",
        Some("Opening a definition does not change BIN bytes."));
    BrowserSearch => (
        "Open advanced search",
        "Opens the separate search window for exact, wildcard, and BIN-value searches.",
        "Use it when a quick list filter is not enough.",
        Some("Value search needs a BIN to be loaded."));
    BrowserExpandCollapseAll => (
        "Expand or fold every category",
        "Shows all category contents or folds them to make the list shorter.",
        "Use it to quickly change how much of the parameter tree is visible.",
        Some("This changes the browser view only."));
    BrowserOrganization => (
        "Choose how the list is grouped",
        "Changes whether parameters are grouped by category, favorites, or recent items.",
        "Use it when a different grouping makes the map you want easier to spot.",
        None);
    BrowserSort => (
        "Sort the parameter list",
        "Puts maps in an order such as title, size, type, or address.",
        "Use it when you want similar maps or nearby addresses beside each other.",
        None);
    BrowserSortDirection => (
        "Reverse the list order",
        "Switches between sorting from first to last and last to first.",
        "Use it when the item you want is easier to find at the opposite end.",
        None);
    BrowserFavorite => (
        "Pin a favorite map",
        "Adds or removes this map from your Favorites list.",
        "Use Favorites for maps you open often.",
        Some("A favorite is a shortcut; it does not copy or change the map."));
    BrowserRecent => (
        "Open a recent file or map",
        "Shows recently used BINs, XDFs, or maps so you can return to them quickly.",
        "Use this when you remember working with a file but do not remember where it is saved.",
        Some("Unavailable files are marked missing and cannot be opened."));
    CategoryExpandCollapse => (
        "Open or fold a category",
        "Shows or hides the maps inside this folder in the list.",
        "Use it to make a busy list easier to scan.",
        Some("Folding a folder only changes the view; it does not remove anything."));
    InspectorAxis => (
        "Read an axis value",
        "An axis is a set of labels or numbers that tells what a table's rows or columns mean.",
        "Use the Inspector to understand how this table is organized.",
        Some("A missing or unclear BIN mapping may make an axis unavailable."));
    InspectorPanel => (
        "Show or hide the Inspector",
        "Expands or minimizes the side panel with details for the selected parameter.",
        "Use it when you need more room for the main editor or want to check map details.",
        Some("This changes the layout only."));
    WorkspaceSelector => (
        "Switch workspaces",
        "Changes to another saved arrangement of the same project's windows.",
        "Use it to keep different tasks organized without reopening every table.",
        Some("A workspace changes window layout, not which BIN or XDF is loaded."));
    StartupRestore => (
        "Choose which project files to reopen",
        "Loads the remembered BIN with its XDF, loads only the BIN, or starts without either file.",
        "Choose BIN + XDF when the remembered definition belongs to that BIN.",
        Some("Opening remembered files does not edit them."));
    XdfReuseChoice => (
        "Choose an XDF for this BIN",
        "Loads the last XDF used with this BIN, keeps the current definitions, or opens another XDF.",
        "Use the remembered file only when it matches this calibration BIN.",
        Some("The prompt does not change BIN bytes."));
    WorkspaceQuickSwitch => (
        "Jump to a table",
        "Type part of an open table's title, then choose Go to make it active.",
        "Use Quick when several table windows are open.",
        Some("This changes focus; it does not edit the selected map."));
    TableArrangement => (
        "Arrange table windows",
        "Stacks, tiles, closes extra, or resets open table windows.",
        "Use it to quickly tidy the current workspace.",
        Some("Resetting table positions does not change BIN values."));
    WindowDock => (
        "Focus or minimize a window",
        "Selects an open tool or table; selecting the focused one again minimizes it.",
        "Use the bottom dock to find windows that are open or minimized.",
        Some("Right-click a dock tab for additional window actions."));
    WorkspaceSnapshot => (
        "Save or manage a workspace",
        "A saved workspace remembers open tools and where their windows were placed.",
        "Use it when you want to come back to this arrangement later.",
        Some("Workspace snapshots do not save BIN edits or replace project files."));
    SettingsAppearance => (
        "Change an appearance setting",
        "Adjusts the app's layout, theme, density, or visible panels.",
        "Use settings to make the interface easier to read and navigate.",
        Some("These choices change the interface, not BIN or XDF contents."));
    SettingsReset => (
        "Reset app preferences",
        "Restores app-wide appearance, browser, and shortcut preferences to their defaults.",
        "Use it if you want to start personalizing the interface again.",
        Some("This does not edit BIN or XDF files."));
    ShortcutRecorder => (
        "Record a keyboard shortcut",
        "Waits for you to press a key chord and assigns it to this command.",
        "Choose a shortcut you can remember and that is not already in use.",
        Some("A conflict is rejected without changing either command."));
    ShortcutSearch => (
        "Find a command",
        "Filters the shortcut list by command name, category, ID, or current shortcut.",
        "Use it to find a command before changing its key binding.",
        Some("Filtering only hides rows; it does not change shortcuts."));
    CommandPaletteSearch => (
        "Find an app command",
        "Filters available actions by command name, category, ID, or description.",
        "Use it when you know what you want to do but cannot remember where the command lives.",
        Some("Disabled commands explain what is missing when you hover them."));
    ShortcutBinding => (
        "Clear or reset a shortcut",
        "Clear leaves a command unbound; Reset restores its default shortcut.",
        "Use it to resolve conflicts or remove a shortcut you do not use.",
        Some("These actions change TunerNook's key bindings only."));
    DebugReportAction => (
        "Copy or export a debug report",
        "Copies diagnostic details to the clipboard or saves them as a separate report file.",
        "Use it when asking for help with an app problem.",
        Some("The report can include file paths and app state; review it before sharing."));
    DebugReportText => (
        "Read diagnostic details",
        "Shows technical information about the current app session and loaded documents.",
        "Use it to help explain a reproducible issue.",
        Some("It is a diagnostic view, not a BIN or XDF editor."));
    SchoolMeToggle => (
        "Turn School-Me Mode on or off",
        "Shows large, plain-language explanations when you hover or focus app controls.",
        "Turn it on while learning the app, then turn it off whenever you prefer a quieter screen.",
        Some("Help bubbles explain controls; they do not make edits or approve agent proposals."));
    SweepToggle => (
        "Turn Sweep on or off",
        "Animates the selected table cells and graph points to make the active range easier to see.",
        "Use it as a visual aid when you are working with a selection.",
        Some("Sweep is only a display effect; it does not change values."));
    WorkspaceBackground => (
        "Choose a workspace background",
        "Places a picture behind floating tables on this numbered workspace.",
        "Use it as a visual reminder or to make a workspace easier to recognize.",
        Some("The picture is only decoration; it never changes BIN or XDF data."));
    ProjectNotepad => (
        "Write a project note",
        "Keeps plain-text reminders with the active BIN project.",
        "Use it for TODOs, map notes, or ideas you want to remember for this file.",
        Some("Notes are not written into the BIN or XDF."));
    NotepadStayOnTop => (
        "Keep the note in front",
        "Keeps the notepad above other TunerNook windows while it is open.",
        "Use it when you want to read notes while working on a table.",
        Some("This only changes TunerNook window order, not other desktop apps."));
    TableCell => (
        "Select a table cell",
        "A cell is one number in the map. Clicking it selects that number for the editor.",
        "Use it before typing a value or checking its raw bytes.",
        Some("The displayed number may be converted from the bytes stored in the BIN."));
    TableActivate => (
        "Activate this table",
        "Brings this table forward and makes it the active table for editing shortcuts.",
        "Use it when a different open table should receive your edits or keyboard actions.",
        Some("Only the active table receives table-edit shortcuts."));
    TableOpenSurface => (
        "Open the 3D surface",
        "Opens an interactive graph of this table's values.",
        "Use it to inspect the map shape or select and compare points.",
        Some("Changing the view does not edit table values."));
    TableRange => (
        "Select several cells",
        "Drag across cells to select a rectangle; copy and paste can work with the whole group.",
        "Use a range when several nearby numbers should be copied or changed together.",
        Some("Check the highlighted cells before applying a change."));
    TableAxis => (
        "Select an axis number",
        "Reads the real value behind a row or column label, when the XDF describes one.",
        "Use it when you need to inspect or edit the table's breakpoints.",
        Some("A label is not editable if the XDF does not map it to BIN bytes."));
    EngineeringValue => (
        "Type an engineering value",
        "This is the human-friendly value for the selected cell, such as pressure or speed.",
        "Use it when you want to enter a real-world value instead of raw bytes.",
        Some("The XDF conversion decides how this number becomes stored bytes."));
    ParameterRawValue => (
        "Type a stored raw value",
        "Enters the number or hexadecimal bits that are stored for this scalar or flag.",
        "Use it when you know the storage value rather than its engineering conversion.",
        Some("Check the selected parameter's format and signedness before applying."));
    ParameterBitToggle => (
        "Toggle a bit",
        "Turns one stored flag bit on or off in the working copy.",
        "Use it when changing a specific documented flag or bitfield.",
        Some("Verify the bit's meaning first; a label does not guarantee its effect."));
    ApplyValue => (
        "Apply the typed value",
        "Writes the typed value into the open BIN working copy for the selected cell or range.",
        "Use it after checking the target and the engineering value.",
        Some("This changes the in-memory copy and can be undone; Save As creates an output file."));
    TableZoom => (
        "Zoom the table",
        "Makes the table text, cells, and controls larger or smaller together.",
        "Use it to fit a dense table or make values easier to read.",
        None);
    TablePrecision => (
        "Choose decimal places",
        "Controls how many digits appear after the decimal point in this table.",
        "Use more digits when small differences matter, or fewer for a cleaner view.",
        Some("Rounding the display does not change the stored value."));
    TableColoring => (
        "Color values by range",
        "Uses color to help show which cells are lower, middle, or higher.",
        "Use it to spot patterns across a table.",
        Some("Colors describe the visible values; they are not a safety rating."));
    TableQuickAction => (
        "Use a table shortcut",
        "Copies, pastes, undoes, redoes, or applies a table action using the selected cells.",
        "Use the letters and tooltips to learn what each small shortcut button does.",
        Some("Check the selected range before pasting or applying changes."));
    CellTransformInput => (
        "Enter a value for a cell operation",
        "Provides the engineering number used by Fill, Add, or Multiply on the selected cells.",
        "Use the operation that matches the change you intend to make.",
        Some("Check the highlighted selection and make sure the value is in the displayed units."));
    CellClampRange => (
        "Set minimum and maximum limits",
        "Clamps every selected engineering value so it stays between the two limits.",
        "Enter a minimum and maximum separated by a comma.",
        Some("The minimum cannot be greater than the maximum; changes are undoable."));
    SurfaceGraph => (
        "Use the 3D surface",
        "Right-drag rotates, middle-drag pans, and the mouse wheel zooms. Left-drag can select cells or move selected values.",
        "Use the graph to inspect the map shape and connect points to table cells.",
        Some("Dragging points edits the in-memory BIN working copy; review changes before saving."));
    SurfaceResetView => (
        "Reset the graph view",
        "Restores the surface camera, zoom, pan, and range controls to their defaults.",
        "Use it when the map is off screen or difficult to read.",
        Some("This resets the view only, not calibration values."));
    SurfaceHeight => (
        "Change graph height exaggeration",
        "Makes vertical value differences look taller or flatter in the 3D picture.",
        "Increase it when small variations are hard to see.",
        Some("Height exaggeration changes only the graph, not stored values."));
    SurfaceAutoRange => (
        "Fit the graph to its values",
        "Chooses graph height limits from the map's current numbers.",
        "Use it when the surface looks too flat or is cut off.",
        Some("Automatic range changes the picture, not the table."));
    SurfaceAxes => (
        "Show graph axes",
        "Shows the numbered directions around the 3D map.",
        "Use it when you need to connect graph locations to table rows and columns.",
        None);
    SurfaceWireframe => (
        "Show the graph grid",
        "Draws lines across the surface so you can see the map's shape.",
        "Use it to follow how values change between points.",
        Some("The wireframe is only a drawing of the same table."));
    SurfacePoint => (
        "Select a graph point",
        "A point represents one cell in the matching table.",
        "Use it to find and edit the same cell in the table.",
        Some("Dragging selected points changes their in-memory values; review before saving."));
    SurfaceSmoothPull => (
        "Smoothly pull nearby points",
        "With the smooth-pull key held, nearby selected points move less than the point under the mouse.",
        "Use it for a gentle shape change across selected map cells.",
        Some("Only selected points move; check the linked table cells before saving."));
    SearchQuery => (
        "Search for parameters",
        "Looks through map names, IDs, categories, or values for what you typed.",
        "Use words or the supported wildcard marks to narrow a large list.",
        Some("Searching cell values requires a BIN to be loaded."));
    SearchField => (
        "Choose what to search",
        "Selects whether the search checks names, IDs, categories, raw values, or engineering values.",
        "Use the field that matches the kind of clue you have.",
        Some("Value searches need a loaded BIN and the correct XDF mapping for engineering values."));
    SearchMatchMode => (
        "Choose how text matches",
        "Controls whether a query is exact, contains text, or uses wildcard symbols.",
        "Use exact mode for a known full value, or contains mode for a partial name.",
        Some("Wildcards change matching rules; they do not search the internet."));
    SearchSort => (
        "Sort search results",
        "Changes the order of matching parameters.",
        "Use it to put the result you need near the top.",
        None);
    SearchResult => (
        "Open a search result",
        "Selects the matching parameter and can open its table for inspection.",
        "Use it after checking the result's name, category, and table size.",
        Some("A name match alone does not prove a map is the one you intended."));
    MapFinderGridRange => (
        "Choose map dimensions",
        "Sets the row and column sizes the scanner will look for.",
        "Use a narrow range to scan faster when you know the map's approximate shape.",
        Some("A detected grid is only a candidate; verify its values and location."));
    MapFinderScore => (
        "Set the candidate score cutoff",
        "Hides candidate grids below the selected heuristic score.",
        "Raise the cutoff to see fewer, stronger-looking candidates.",
        Some("A high score is not proof that a candidate is a real map."));
    MapFinderColoring => (
        "Color candidate values",
        "Colors grid values to make patterns and ranges easier to see.",
        "Use it to spot changes across the candidate preview.",
        Some("Color does not confirm that a candidate is a calibration map."));
    MapFinderPrecision => (
        "Choose candidate decimal places",
        "Changes how many decimal digits are shown in the candidate preview.",
        "Use extra digits when two nearby values look the same after rounding.",
        Some("Display rounding does not change the BIN bytes."));
    MapFinderZoom => (
        "Zoom the candidate preview",
        "Changes the size of the Map Finder controls and candidate grid on screen.",
        "Use it to fit more values or make a dense preview easier to read.",
        Some("Zoom changes the display only."));
    MapFinderFormat => (
        "Choose number format",
        "Tells the scanner how many bytes make a number and how to interpret them.",
        "Use the format that fits the ECU data you are inspecting.",
        Some("A wrong format can turn useful bytes into huge or nonsense numbers."));
    MapFinderEndian => (
        "Choose byte order",
        "Tells the app which byte comes first when several bytes form one number.",
        "Try the order known for this ECU or compare candidates carefully.",
        Some("Wrong byte order can make a real map look like random values."));
    MapFinderAddressRange => (
        "Limit the scan range",
        "Tells Map Finder which BIN addresses it may inspect.",
        "Use a smaller range when you have a likely region to check.",
        Some("Addresses are positions in the BIN; verify them before defining a map."));
    MapFinderSkipMapped => (
        "Skip already mapped bytes",
        "Avoids scanning BIN areas that the current XDF already describes.",
        "Leave this on to reduce duplicate candidates when an XDF is loaded.",
        None);
    MapFinderUnaligned => (
        "Check every byte offset",
        "Scans positions between usual number boundaries, which can find unusual layouts.",
        "Use it when a normal scan misses a map.",
        Some("This can make a large scan take longer."));
    MapFinderScanRange => (
        "Scan the selected range",
        "Looks for map-shaped number grids within the chosen BIN address range.",
        "Use it for a faster focused search.",
        Some("Results are guesses and must be checked before creating an XDF definition."));
    MapFinderScanAll => (
        "Scan the whole BIN",
        "Searches all eligible BIN regions using the selected format and size limits.",
        "Use it when you do not know where the map is.",
        Some("A full scan can take longer; you can stop it and discard partial results."));
    MapFinderStop => (
        "Stop the map scan",
        "Asks the background scan to stop and throws away partial candidates.",
        "Use it if the scan is taking too long or you want to change settings.",
        Some("Stopping does not edit the BIN or XDF."));
    MapFinderCandidate => (
        "Inspect a map candidate",
        "Shows a grid that looks map-like according to the scanner's checks.",
        "Use the values and pattern as clues before deciding what the grid means.",
        Some("A high score is not proof that the candidate is a real calibration map."));
    MapFinderAxis => (
        "Choose an axis suggestion",
        "Connects possible row or column values to a candidate map.",
        "Use an axis only after checking that its values match the map shape.",
        Some("Axis suggestions are guesses and may be stored far from the map bytes."));
    MapFinderCreateXdf => (
        "Create an XDF definition from this candidate",
        "Starts a reviewed draft that describes the candidate's address, size, and format.",
        "Use it after verifying the candidate and any axis suggestions.",
        Some("The draft is not saved until you choose Save XDF As."));
    MapFinderInspectHex => (
        "Inspect this candidate in Hex Editor",
        "Jumps to the candidate's BIN address so you can check its raw bytes.",
        "Use it to verify the candidate's location and number format.",
        Some("Raw bytes can look different depending on format and byte order."));
    HexAddress => (
        "Go to a BIN address",
        "Moves the Hex Editor to the requested byte position.",
        "Use it when you know an address from an XDF or Map Finder candidate.",
        Some("Addresses start at the beginning of the loaded BIN."));
    HexFormat => (
        "Read bytes as a number format",
        "Groups bytes into numbers such as 8-bit, 16-bit, 32-bit, or floating-point values.",
        "Use a format that matches the ECU data definition.",
        Some("A wrong choice changes the displayed number, not the underlying bytes."));
    HexEndian => (
        "Choose byte order",
        "Controls which byte is treated as the first part of a multi-byte number.",
        "Use the order specified by the XDF or confirmed from known data.",
        Some("If values look wildly wrong, check byte order and format."));
    HexSearch => (
        "Find a value in the BIN",
        "Searches the loaded BIN using the current number format and byte order.",
        "Use Previous and Next to move between matches.",
        Some("The search uses the selected interpretation; it may find a byte pattern, not a real map."));
    HexQuickAction => (
        "Use a Hex Editor shortcut",
        "Copies selected bytes, pastes hexadecimal text, or undoes/redoes a BIN edit.",
        "Use the small buttons for frequent byte-editing actions.",
        Some("Check the selected bytes and undo history before applying edits."));
    HexPrecision => (
        "Choose displayed decimal places",
        "Changes how many digits after the decimal point are shown for floating-point values.",
        "Use more digits to inspect small differences.",
        Some("Changing display precision does not change the raw bytes."));
    HexColoring => (
        "Color visible number values",
        "Uses a color range based on visible values to help spot patterns.",
        "Use it when a large BIN has outliers that make one whole-file color range hard to read.",
        Some("Colors are a visual aid, not a definition or safety check."));
    HexSelection => (
        "Select bytes or numbers",
        "Highlights the current byte or range so you can inspect or copy it.",
        "Use dragging or the keyboard to select a range.",
        Some("Check the selected address and format before editing."));
    HexRawEdit => (
        "Edit the selected raw value",
        "Changes the selected bytes in the open BIN working copy.",
        "Use it only when you understand the format and exact byte range.",
        Some("Edits can affect ECU behavior. The source file is not saved unless you choose Save As."));
    HexCreateXdf => (
        "Create an XDF definition from this selection",
        "Opens an XDF draft prefilled with the selected address and value format.",
        "Use it after checking the bytes and choosing the right map dimensions.",
        Some("The definition remains a draft until you explicitly save it."));
    CompareFilter => (
        "Filter compared parameters",
        "Shows only map names or IDs that match your filter text.",
        "Use it to focus on one group of maps.",
        None);
    CompareChangedOnly => (
        "Show only changed maps",
        "Hides maps whose compared values are the same.",
        "Use it to focus on differences between the source and destination BINs.",
        None);
    CompareValueMode => (
        "Choose how to compare values",
        "Switches between raw numbers, engineering values, and difference views.",
        "Use the view that makes the change easiest to understand.",
        Some("Engineering values depend on the XDF's conversion."));
    CompareCell => (
        "Select a compared cell",
        "Highlights the same row and column in both maps so you can inspect the difference.",
        "Use it before building a transfer plan for matching maps.",
        Some("Matching names do not guarantee identical BIN layouts."));
    CompareTransfer => (
        "Plan a map transfer",
        "Previews copying selected map values from one BIN to another.",
        "Use it after checking map identity, size, and exact changes.",
        Some("Review the plan before applying; the destination working copy changes."));
    CompareGraph => (
        "Compare maps as a graph",
        "Shows how source and destination map values differ across the surface.",
        "Use it to spot broad changes and local spikes.",
        Some("The graph is a view of the comparison; inspect exact cells before editing."));
    XdfMetadata => (
        "Edit XDF description details",
        "Changes information such as a parameter's title, category, or description.",
        "Use it to make a definition easier to recognize.",
        Some("Metadata labels do not change BIN bytes."));
    XdfStorage => (
        "Describe where and how a table is stored",
        "Sets the XDF address, dimensions, number format, byte order, and spacing.",
        "Use it when creating or correcting a table definition.",
        Some("A wrong address or format can make the table show unrelated bytes."));
    XdfConversion => (
        "Describe how raw numbers become real units",
        "Sets the formula that turns stored bytes into displayed engineering values.",
        "Use it when the raw number needs a scale or offset.",
        Some("Check the formula and units against reliable information before editing."));
    XdfAxis => (
        "Describe a table axis",
        "Defines the labels or stored values used for table rows or columns.",
        "Use it when a map's breakpoints are known and can be verified.",
        Some("An axis address can be separate from the table data address."));
    XdfParameterAction => (
        "Manage an XDF definition",
        "Adds, edits, copies, reorders, or removes a definition in the current draft.",
        "Use it to organize or correct how maps are described.",
        Some("Review draft changes carefully; the original XDF is not overwritten."));
    XdfSave => (
        "Save an XDF draft as a new file",
        "Writes the reviewed definition draft to a new XDF path.",
        "Use it when the draft is complete and validated.",
        Some("Double-check the destination path so you keep the source definition."));
    NookLinkConnection => (
        "Connect your chosen agent to NookLink",
        "Shows how an external agent can use TunerNook's local control connection.",
        "Use it after you configure the agent yourself.",
        Some("TunerNook does not install or launch an agent; keep the connection token private."));
    NookLinkStarter => (
        "Copy the agent starter prompt",
        "Copies a provider-neutral message that helps your chosen agent learn how to work with TunerNook.",
        "Use it in the agent's own chat after attaching the context file.",
        Some("The prompt does not configure or start the external agent."));
    NookLinkContext => (
        "Save the NookLink context file",
        "Exports a short guide that explains the app and the agent-control safety limits.",
        "Attach it to the conversation with your configured agent.",
        Some("The file describes capabilities; it does not grant filesystem or shell access."));
    NookLinkTask => (
        "Start or review an agent task",
        "Shows the goal, progress, and findings sent by the external agent.",
        "Use it for a specific job you asked the agent to do.",
        Some("Agent reports are suggestions until you review and approve them."));
    NookLinkChallenge => (
        "Confirm a protected action",
        "Asks you to complete an in-app check before a long task, raw read, apply, or save.",
        "Use it only after reading the exact action and target shown in TunerNook.",
        Some("The challenge is a consent checkpoint, not proof that software changes are safe."));
    NookLinkReview => (
        "Review an agent proposal",
        "Shows the exact findings and proposed changes before you choose what to do.",
        "Compare the evidence with the app's current BIN and XDF before approval.",
        Some("Review does not save the original file; output saving is a separate action."));
    NookLinkCancel => (
        "Cancel an agent task",
        "Marks the task cancelled so the agent can see it on its next request.",
        "Use it if the goal changed or the agent should stop working through NookLink.",
        Some("This cannot stop the agent's separate work or independent computer access."));
}

pub(crate) fn command_help(descriptor: &CommandDescriptor) -> SchoolHelpCopy<'_> {
    let category_hint = match descriptor.category.as_str() {
        "File" => "Use this when you want to open, save, or manage a project file.",
        "Edit" => {
            "Use this when you want to change data in the open working copy or undo a change."
        }
        "View" => "Use this when you want to change what TunerNook shows.",
        "Workspace" => "Use this when you want to arrange windows or switch layouts.",
        "Tools" => "Use this when you need one of TunerNook's specialized tools.",
        _ => "Use this when you want the action described above.",
    };
    let (what_it_does, keep_in_mind) = match descriptor.id.as_str() {
        "file.open-bin" => (
            "Opens a BIN calibration file as a working copy for viewing and editing.",
            Some("Opening it does not overwrite the source BIN."),
        ),
        "file.open-xdf" => (
            "Opens an XDF guide that describes where maps and values are in a BIN.",
            Some("Use the XDF that matches the BIN; a mismatch can show wrong values."),
        ),
        "file.save-as" => (
            "Saves the open BIN working copy to a new output file.",
            Some("Check the destination path; the source BIN is not silently overwritten."),
        ),
        "file.save-xdf-as" => (
            "Saves the current XDF definition or draft to an output file.",
            Some("Check the destination path and review the definition before saving."),
        ),
        "edit.apply-cell" => (
            "Applies the typed engineering value to the selected cell or range in the open BIN working copy.",
            Some("The source file is unchanged until you explicitly choose Save As."),
        ),
        "edit.paste-cells" => (
            "Pastes copied cell values into the selected range in the open BIN working copy.",
            Some("Check the highlighted cells before pasting; you can undo an in-memory edit."),
        ),
        "view.map-finder" => (
            "Scans the BIN for number grids that might be calibration maps.",
            Some("Map Finder candidates are guesses; verify them before making an XDF definition."),
        ),
        "view.hex-editor" => (
            "Shows the BIN bytes and lets you inspect them as numbers or text.",
            Some("A wrong number format or byte order can make bytes look like nonsense."),
        ),
        "tools.xdf-maker-editor" => (
            "Opens a draft editor for descriptions of maps, axes, and other XDF definitions.",
            Some("Changes stay in the draft until you explicitly save an output XDF."),
        ),
        _ => (descriptor.description.as_str(), None),
    };
    SchoolHelpCopy {
        title: &descriptor.label,
        what_it_does,
        when_to_use: Some(category_hint),
        keep_in_mind,
    }
}

pub(crate) fn show_for_response(
    ctx: &egui::Context,
    response: &egui::Response,
    enabled: bool,
    help: &SchoolHelpCopy<'_>,
) {
    if !enabled {
        return;
    }
    let Some(pointer) = ctx.pointer_hover_pos() else {
        return;
    };
    let pointer_targets_response = response.contains_pointer()
        && (response.hovered() || !response.enabled())
        && ctx.layer_id_at(pointer) == Some(response.layer_id);
    if !pointer_targets_response {
        return;
    }

    let state_id = egui::Id::new(HELP_STATE_ID);
    let candidate = OwnedSchoolHelp::from(help);
    ctx.data_mut(|data| {
        let mut state = data
            .get_temp::<SchoolHelpWindowState>(state_id)
            .unwrap_or_default();
        state.candidate = Some(candidate);
        data.insert_temp(state_id, state);
    });
}

pub(crate) fn show_help_window(ctx: &egui::Context, enabled: bool) -> bool {
    let state_id = egui::Id::new(HELP_STATE_ID);
    if !enabled {
        ctx.data_mut(|data| data.insert_temp(state_id, SchoolHelpWindowState::default()));
        return true;
    }

    let mut state = ctx
        .data(|data| data.get_temp::<SchoolHelpWindowState>(state_id))
        .unwrap_or_default();
    if let Some(candidate) = state.candidate.take() {
        state.current = Some(candidate);
    }
    let Some(help) = state.current.clone() else {
        ctx.data_mut(|data| data.insert_temp(state_id, state));
        return true;
    };
    ctx.data_mut(|data| data.insert_temp(state_id, state));

    let viewport = ctx.content_rect();
    let window_size = egui::vec2(
        viewport.width().clamp(1.0, 380.0),
        viewport.height().clamp(1.0, 300.0),
    );
    let default_pos = egui::pos2(
        viewport.right() - window_size.x - 12.0,
        viewport.bottom() - window_size.y - 52.0,
    );
    let mut open = true;
    egui::Window::new("School-Me Help")
        .id(egui::Id::new(HELP_WINDOW_ID))
        .order(egui::Order::Foreground)
        .default_pos(default_pos)
        .default_size(window_size)
        .max_height(viewport.height())
        .constrain_to(viewport)
        .resizable(true)
        .collapsible(false)
        .open(&mut open)
        .show(ctx, |ui| {
            let max_height = (viewport.height() - 88.0).clamp(48.0, 260.0);
            let _scroll = egui::ScrollArea::vertical()
                .id_salt("school-me-help-scroll")
                .max_height(max_height)
                .show(ui, |ui| {
                    ui.heading(&help.title);
                    ui.strong("What it does");
                    ui.label(&help.what_it_does);
                    if let Some(when) = help.when_to_use.as_deref() {
                        ui.add_space(4.0);
                        ui.strong("When to use it");
                        ui.label(when);
                    }
                    if let Some(keep) = help.keep_in_mind.as_deref() {
                        ui.add_space(4.0);
                        ui.colored_label(ui.visuals().warn_fg_color, "Keep in mind");
                        ui.label(keep);
                    }
                });
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(egui::Id::new("school-me-help-test-scroll-id"), _scroll.id);
            });
        });
    open
}

pub(crate) fn show_custom_for_response(
    ctx: &egui::Context,
    response: &egui::Response,
    enabled: bool,
    id: SchoolHelpId,
) {
    show_custom_for_response_with_reason(ctx, response, enabled, id, None);
}

pub(crate) fn show_custom_for_response_with_reason(
    ctx: &egui::Context,
    response: &egui::Response,
    enabled: bool,
    id: SchoolHelpId,
    reason: Option<&str>,
) {
    let mut help = *custom_help(id);
    if let Some(reason) = reason {
        help.keep_in_mind = Some(reason);
    }
    show_for_response(ctx, response, enabled, &help);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::egui;
    use crate::CommandRegistry;

    fn rendered_text_rect(output: &egui::FullOutput, needle: &str) -> Option<egui::Rect> {
        fn find(shape: &egui::Shape, needle: &str) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Text(text) if text.galley.job.text.contains(needle) => {
                    Some(text.visual_bounding_rect())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, needle)),
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|clipped| find(&clipped.shape, needle))
    }

    fn help_window_rect(context: &egui::Context) -> Option<egui::Rect> {
        context.memory(|memory| memory.area_rect(egui::Id::new(HELP_WINDOW_ID)))
    }

    fn render_one(
        context: &egui::Context,
        screen: egui::Rect,
        time: f64,
        pointer: Option<egui::Pos2>,
        at_edge: bool,
        events: Vec<egui::Event>,
        help: &SchoolHelpCopy<'_>,
    ) -> (egui::FullOutput, egui::Rect) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        if let Some(pointer) = pointer {
            input.events.push(egui::Event::PointerMoved(pointer));
        }

        let mut target_rect = None;
        let output = context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = if at_edge {
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(screen.right() - 40.0, screen.top() + 40.0),
                        egui::vec2(24.0, 24.0),
                    );
                    ui.put(rect, egui::Button::new("Help target"))
                } else {
                    ui.button("Help target")
                };
                target_rect = Some(response.rect);
                show_for_response(ui.ctx(), &response, true, help);
            });
            show_help_window(ctx, true);
        });
        (output, target_rect.expect("help target should be rendered"))
    }

    #[test]
    fn school_help_command_help_covers_all_registered_commands() {
        let descriptors = CommandRegistry::core().all_descriptors();
        assert!(!descriptors.is_empty());
        for descriptor in descriptors {
            let help = command_help(&descriptor);
            assert!(
                !help.title.trim().is_empty(),
                "{} lacks a title",
                descriptor.id
            );
            assert!(
                !help.what_it_does.trim().is_empty(),
                "{} lacks an explanation",
                descriptor.id
            );
            assert!(
                help.when_to_use.is_some_and(|copy| !copy.trim().is_empty()),
                "{} lacks a use hint",
                descriptor.id
            );
        }
    }

    #[test]
    fn school_help_custom_catalog_covers_every_id_with_complete_copy() {
        assert!(!ALL_CUSTOM_HELP_IDS.is_empty());
        for id in ALL_CUSTOM_HELP_IDS {
            let help = custom_help(*id);
            assert!(!help.title.trim().is_empty(), "{id:?} lacks a title");
            assert!(
                !help.what_it_does.trim().is_empty(),
                "{id:?} lacks an explanation"
            );
            assert!(
                help.when_to_use.is_some_and(|copy| !copy.trim().is_empty())
                    || help
                        .keep_in_mind
                        .is_some_and(|copy| !copy.trim().is_empty()),
                "{id:?} needs at least one detail section"
            );
        }
    }

    #[test]
    fn school_help_hover_does_not_spawn_a_control_anchored_popup() {
        let help = custom_help(ALL_CUSTOM_HELP_IDS[0]);
        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        let (warmup, target) = render_one(&context, screen, 1.0, None, false, Vec::new(), help);
        warmup.drop_without_applying_deltas();
        let (hovered, _) = render_one(
            &context,
            screen,
            1.01,
            Some(target.center()),
            false,
            Vec::new(),
            help,
        );
        hovered.drop_without_applying_deltas();

        let tooltip_layers = context.memory(|memory| {
            memory
                .layer_ids()
                .filter(|layer| {
                    layer.order == egui::Order::Tooltip && layer.id != egui::Id::new(HELP_WINDOW_ID)
                })
                .count()
        });
        let help_window = help_window_rect(&context);
        assert_eq!(
            tooltip_layers, 0,
            "School-Me help should use one shared window, not per-control tooltip popups"
        );
        assert!(
            help_window.is_some(),
            "hover should open the shared help window"
        );
    }

    #[test]
    fn school_help_window_refreshes_and_keeps_its_last_explanation() {
        let first = custom_help(ALL_CUSTOM_HELP_IDS[0]);
        let second = custom_help(ALL_CUSTOM_HELP_IDS[1]);
        assert_ne!(first.title, second.title);
        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        let draw = |ui: &mut egui::Ui, targets: &mut [Option<egui::Rect>; 2]| {
            let ctx = ui.ctx().clone();
            egui::CentralPanel::default().show(ui, |ui| {
                let first_response = ui.button("First target");
                targets[0] = Some(first_response.rect);
                show_for_response(ui.ctx(), &first_response, true, first);
                let second_response = ui.button("Second target");
                targets[1] = Some(second_response.rect);
                show_for_response(ui.ctx(), &second_response, true, second);
            });
            show_help_window(&ctx, true);
        };

        let mut targets = [None, None];
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.0);
        let output = context.run_ui(input, |ctx| draw(ctx, &mut targets));
        output.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.01);
        input.events = vec![egui::Event::PointerMoved(
            targets[0].expect("first target should render").center(),
        )];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut targets));
        output.drop_without_applying_deltas();
        let first_window_rect = help_window_rect(&context).expect("help window should open");

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.015);
        input.events = vec![egui::Event::PointerMoved(
            targets[0].expect("first target should render").center(),
        )];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut targets));
        let first_visible = rendered_text_rect(&output, first.title).is_some();
        output.drop_without_applying_deltas();
        assert!(
            first_visible,
            "the shared window should show the hovered explanation"
        );

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.02);
        input.events = vec![egui::Event::PointerMoved(
            targets[1].expect("second target should render").center(),
        )];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut targets));
        let first_stale = rendered_text_rect(&output, first.title).is_some();
        let second_visible = rendered_text_rect(&output, second.title).is_some();
        output.drop_without_applying_deltas();
        assert!(!first_stale, "the shared window should replace old content");
        assert!(second_visible);
        assert_eq!(
            help_window_rect(&context)
                .expect("the same help window should remain open")
                .min,
            first_window_rect.min,
            "content refresh should preserve the window position"
        );

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.03);
        input.events = vec![egui::Event::PointerMoved(egui::pos2(200.0, 460.0))];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut targets));
        let retained = rendered_text_rect(&output, second.title).is_some();
        output.drop_without_applying_deltas();
        assert!(retained, "blank canvas should not erase readable help");
        let window_center = help_window_rect(&context)
            .expect("the same help window should remain open")
            .center();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.04);
        input.events = vec![egui::Event::PointerMoved(window_center)];
        let output = context.run_ui(input, |ui| draw(ui, &mut targets));
        let readable = rendered_text_rect(&output, second.title).is_some();
        output.drop_without_applying_deltas();
        assert!(
            readable,
            "moving the pointer into help must preserve its content"
        );
        assert_eq!(
            context.memory(|memory| {
                memory
                    .layer_ids()
                    .filter(|layer| layer.id == egui::Id::new(HELP_WINDOW_ID))
                    .count()
            }),
            1,
            "content refreshes inside one persistent help window"
        );
    }

    #[test]
    fn school_help_is_rendered_once_at_the_end_of_the_app_frame() {
        let mut app = crate::TunerApp::headless();
        app.preferences.school_me_mode = true;
        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
        let mut frame = eframe::Frame::_new_kittest();
        let render =
            |input: egui::RawInput, app: &mut crate::TunerApp, frame: &mut eframe::Frame| {
                context.run_ui(input, |ui| eframe::App::ui(app, ui, frame))
            };

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.0);
        let output = render(input, &mut app, &mut frame);
        let filter = rendered_text_rect(&output, "Title, ID, category…")
            .expect("the parameter filter should render");
        output.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.01);
        input.events = vec![egui::Event::PointerMoved(filter.center())];
        let output = render(input, &mut app, &mut frame);
        output.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.3);
        input.events = vec![egui::Event::PointerMoved(filter.center())];
        let output = render(input, &mut app, &mut frame);
        let filter_help_visible =
            rendered_text_rect(&output, "Filter the parameter list").is_some();
        output.drop_without_applying_deltas();
        assert!(filter_help_visible);
        assert_eq!(
            context.memory(|memory| {
                memory
                    .layer_ids()
                    .filter(|layer| layer.id == egui::Id::new(HELP_WINDOW_ID))
                    .count()
            }),
            1,
            "the real app frame should render exactly one shared help window"
        );
    }

    #[test]
    fn school_help_uses_only_the_topmost_hovered_window_layer() {
        assert!(ALL_CUSTOM_HELP_IDS.len() >= 2);
        let hidden_help = custom_help(ALL_CUSTOM_HELP_IDS[0]);
        let visible_help = custom_help(ALL_CUSTOM_HELP_IDS[1]);
        assert_ne!(hidden_help.title, visible_help.title);

        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        let mut hidden_rect = None;
        let mut visible_rect = None;
        let draw = |ctx: &egui::Context,
                    hidden_rect: &mut Option<egui::Rect>,
                    visible_rect: &mut Option<egui::Rect>| {
            egui::Window::new("Covered window")
                .id(egui::Id::new("school-help-background-window"))
                .order(egui::Order::Middle)
                .default_pos(egui::pos2(40.0, 40.0))
                .default_size(egui::vec2(220.0, 100.0))
                .show(ctx, |ui| {
                    let response = ui.button("Covered window target");
                    *hidden_rect = Some(response.rect);
                    response.request_focus();
                    show_for_response(ui.ctx(), &response, true, hidden_help);
                });
            egui::Window::new("Visible window")
                .id(egui::Id::new("school-help-foreground-window"))
                .order(egui::Order::Middle)
                .default_pos(egui::pos2(40.0, 40.0))
                .default_size(egui::vec2(220.0, 100.0))
                .show(ctx, |ui| {
                    let response = ui.button("Visible window target");
                    *visible_rect = Some(response.rect);
                    show_for_response(ui.ctx(), &response, true, visible_help);
                });
            show_help_window(ctx, true);
        };

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.0);
        let output = context.run_ui(input, |ctx| draw(ctx, &mut hidden_rect, &mut visible_rect));
        output.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.01);
        input.events = vec![egui::Event::PointerMoved(
            visible_rect.expect("visible target should render").center(),
        )];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut hidden_rect, &mut visible_rect));
        output.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.02);
        input.events = vec![egui::Event::PointerMoved(
            visible_rect.expect("visible target should render").center(),
        )];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut hidden_rect, &mut visible_rect));
        output.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.3);
        input.events = vec![egui::Event::PointerMoved(
            visible_rect.expect("visible target should render").center(),
        )];
        let output = context.run_ui(input, |ctx| draw(ctx, &mut hidden_rect, &mut visible_rect));
        let hidden_visible = rendered_text_rect(&output, hidden_help.title).is_some();
        let visible_visible = rendered_text_rect(&output, visible_help.title).is_some();
        output.drop_without_applying_deltas();
        assert!(
            !hidden_visible,
            "an obscured control must never replace the shared help content"
        );
        assert!(
            visible_visible,
            "the visible hovered control should be the one explained"
        );
    }

    #[test]
    fn school_help_window_clamps_and_scrolls() {
        let long_copy = "SCHOOL_ME_SCROLL_MARKER ".repeat(120);
        let help = SchoolHelpCopy {
            title: "Long help card",
            what_it_does: &long_copy,
            when_to_use: None,
            keep_in_mind: None,
        };
        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 220.0));
        let (warmup, target) = render_one(&context, screen, 1.0, None, true, Vec::new(), &help);
        warmup.drop_without_applying_deltas();

        let (opened, _) = render_one(
            &context,
            screen,
            1.01,
            Some(target.center()),
            true,
            Vec::new(),
            &help,
        );
        let window = help_window_rect(&context).expect("hover should open a help window");
        let window_in_viewport = screen.contains_rect(window);
        opened.drop_without_applying_deltas();
        assert!(
            window_in_viewport,
            "help window escaped the viewport: {window:?}"
        );

        let (opened, _) = render_one(
            &context,
            screen,
            1.3,
            Some(target.center()),
            true,
            Vec::new(),
            &help,
        );
        let scroll_id = context
            .data(|data| data.get_temp::<egui::Id>(egui::Id::new("school-me-help-test-scroll-id")))
            .expect("help scroll area should be registered");
        let before_scroll = egui::scroll_area::State::load(&context, scroll_id)
            .map(|state| state.offset.y)
            .unwrap_or_default();
        opened.drop_without_applying_deltas();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.4);
        input.events = vec![
            egui::Event::PointerMoved(window.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -20.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        let after_scroll = context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = ui.put(
                    egui::Rect::from_min_size(
                        screen.max - egui::vec2(40.0, 36.0),
                        egui::vec2(24.0, 24.0),
                    ),
                    egui::Button::new("Help target"),
                );
                show_for_response(ui.ctx(), &response, true, &help);
            });
            show_help_window(ctx, true);
        });
        let after_scroll_offset = egui::scroll_area::State::load(&context, scroll_id)
            .map(|state| state.offset.y)
            .unwrap_or_default();
        after_scroll.drop_without_applying_deltas();
        assert!(
            after_scroll_offset > before_scroll,
            "long help should scroll; before={before_scroll} after={after_scroll_offset}"
        );
    }

    #[test]
    fn school_help_does_not_change_normal_tooltip_timing() {
        let context = egui::Context::default();
        let default_delay = egui::Style::default().interaction.tooltip_delay;
        let help = custom_help(ALL_CUSTOM_HELP_IDS[0]);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        let (output, target) = render_one(&context, screen, 1.0, None, false, Vec::new(), help);
        output.drop_without_applying_deltas();
        let (output, _) = render_one(
            &context,
            screen,
            1.01,
            Some(target.center()),
            false,
            Vec::new(),
            help,
        );
        output.drop_without_applying_deltas();
        assert_eq!(
            context.global_style().interaction.tooltip_delay,
            default_delay
        );
    }
}
