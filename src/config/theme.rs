//! Theme system with 6 built-in themes and 88 color properties.
//!
//! Ported from ref/utils/theme.ts`.
//!
//! Color values are stored as strings in one of two formats:
//! - `"rgb(R,G,B)"` -- explicit 24-bit true-color
//! - `"ansi:COLOR"` -- one of the 16 standard ANSI color names
//!
//! The rendering layer is responsible for translating these strings
//! to actual terminal escape codes.

use serde::{Deserialize, Serialize};

// ============================================================================
// ThemeName / ThemeSetting
// ============================================================================

/// A concrete (resolved) theme name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeName {
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "light-daltonized")]
    LightDaltonized,
    #[serde(rename = "dark-daltonized")]
    DarkDaltonized,
    #[serde(rename = "light-ansi")]
    LightAnsi,
    #[serde(rename = "dark-ansi")]
    DarkAnsi,
}

/// All theme names in definition order.
pub const THEME_NAMES: &[ThemeName] = &[
    ThemeName::Dark,
    ThemeName::Light,
    ThemeName::LightDaltonized,
    ThemeName::DarkDaltonized,
    ThemeName::LightAnsi,
    ThemeName::DarkAnsi,
];

/// A theme preference as stored in user config.
///
/// `Auto` follows the system dark/light mode and is resolved to a
/// concrete `ThemeName` at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeSetting {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "light-daltonized")]
    LightDaltonized,
    #[serde(rename = "dark-daltonized")]
    DarkDaltonized,
    #[serde(rename = "light-ansi")]
    LightAnsi,
    #[serde(rename = "dark-ansi")]
    DarkAnsi,
}

impl ThemeSetting {
    /// Resolve `Auto` to a concrete theme name.
    ///
    /// When `is_dark` is `None` (could not detect), defaults to `Dark`.
    pub fn resolve(self, is_dark: Option<bool>) -> ThemeName {
        match self {
            ThemeSetting::Auto => {
                if is_dark.unwrap_or(true) {
                    ThemeName::Dark
                } else {
                    ThemeName::Light
                }
            }
            ThemeSetting::Dark => ThemeName::Dark,
            ThemeSetting::Light => ThemeName::Light,
            ThemeSetting::LightDaltonized => ThemeName::LightDaltonized,
            ThemeSetting::DarkDaltonized => ThemeName::DarkDaltonized,
            ThemeSetting::LightAnsi => ThemeName::LightAnsi,
            ThemeSetting::DarkAnsi => ThemeName::DarkAnsi,
        }
    }
}

/// All theme setting values (including Auto).
pub const THEME_SETTINGS: &[ThemeSetting] = &[
    ThemeSetting::Auto,
    ThemeSetting::Dark,
    ThemeSetting::Light,
    ThemeSetting::LightDaltonized,
    ThemeSetting::DarkDaltonized,
    ThemeSetting::LightAnsi,
    ThemeSetting::DarkAnsi,
];

// ============================================================================
// Theme struct
// ============================================================================

/// A fully-resolved color palette with all 88 color properties.
///
/// All values are strings in either `"rgb(R,G,B)"` or `"ansi:COLOR"` format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    // -- Core UI ---------------------------------------------------------------
    pub auto_accept: String,
    pub bash_border: String,
    pub primary: String,
    pub primary_shimmer: String,
    pub blue_for_system_spinner: String,
    pub blue_shimmer_for_system_spinner: String,
    pub permission: String,
    pub permission_shimmer: String,
    pub plan_mode: String,
    pub ide: String,
    pub prompt_border: String,
    pub prompt_border_shimmer: String,
    pub text: String,
    pub inverse_text: String,
    pub inactive: String,
    pub inactive_shimmer: String,
    pub subtle: String,
    pub suggestion: String,
    pub remember: String,
    pub background: String,

    // -- Semantic colors -------------------------------------------------------
    pub success: String,
    pub error: String,
    pub warning: String,
    pub merged: String,
    pub warning_shimmer: String,

    // -- Diff colors -----------------------------------------------------------
    pub diff_added: String,
    pub diff_removed: String,
    pub diff_added_dimmed: String,
    pub diff_removed_dimmed: String,
    pub diff_added_word: String,
    pub diff_removed_word: String,

    // -- Agent colors ----------------------------------------------------------
    pub red_for_subagents_only: String,
    pub blue_for_subagents_only: String,
    pub green_for_subagents_only: String,
    pub yellow_for_subagents_only: String,
    pub purple_for_subagents_only: String,
    pub orange_for_subagents_only: String,
    pub pink_for_subagents_only: String,
    pub cyan_for_subagents_only: String,

    // -- Grove / Chrome --------------------------------------------------------
    pub professional_blue: String,
    pub chrome_yellow: String,

    // -- TUI V2 ----------------------------------------------------------------
    pub clawd_body: String,
    pub clawd_background: String,
    pub user_message_background: String,
    pub user_message_background_hover: String,
    pub message_actions_background: String,
    pub selection_bg: String,
    pub bash_message_background_color: String,
    pub memory_background_color: String,
    pub rate_limit_fill: String,
    pub rate_limit_empty: String,
    pub fast_mode: String,
    pub fast_mode_shimmer: String,

    // -- Brief / assistant mode ------------------------------------------------
    pub brief_label_you: String,
    pub brief_label_primary: String,

    // -- Rainbow (ultrathink) --------------------------------------------------
    pub rainbow_red: String,
    pub rainbow_orange: String,
    pub rainbow_yellow: String,
    pub rainbow_green: String,
    pub rainbow_blue: String,
    pub rainbow_indigo: String,
    pub rainbow_violet: String,
    pub rainbow_red_shimmer: String,
    pub rainbow_orange_shimmer: String,
    pub rainbow_yellow_shimmer: String,
    pub rainbow_green_shimmer: String,
    pub rainbow_blue_shimmer: String,
    pub rainbow_indigo_shimmer: String,
    pub rainbow_violet_shimmer: String,
}

// ============================================================================
// Theme lookup
// ============================================================================

/// Get the built-in theme for a given theme name.
pub fn get_theme(name: ThemeName) -> &'static Theme {
    match name {
        ThemeName::Dark => &DARK_THEME,
        ThemeName::Light => &LIGHT_THEME,
        ThemeName::LightDaltonized => &LIGHT_DALTONIZED_THEME,
        ThemeName::DarkDaltonized => &DARK_DALTONIZED_THEME,
        ThemeName::LightAnsi => &LIGHT_ANSI_THEME,
        ThemeName::DarkAnsi => &DARK_ANSI_THEME,
    }
}

// ============================================================================
// Built-in theme definitions
// ============================================================================

macro_rules! define_theme {
    ($name:ident, {
        auto_accept: $auto_accept:expr,
        bash_border: $bash_border:expr,
        primary: $primary:expr,
        primary_shimmer: $primary_shimmer:expr,
        blue_for_system_spinner: $pb_spinner:expr,
        blue_shimmer_for_system_spinner: $pb_shimmer_spinner:expr,
        permission: $permission:expr,
        permission_shimmer: $permission_shimmer:expr,
        plan_mode: $plan_mode:expr,
        ide: $ide:expr,
        prompt_border: $prompt_border:expr,
        prompt_border_shimmer: $prompt_border_shimmer:expr,
        text: $text:expr,
        inverse_text: $inverse_text:expr,
        inactive: $inactive:expr,
        inactive_shimmer: $inactive_shimmer:expr,
        subtle: $subtle:expr,
        suggestion: $suggestion:expr,
        remember: $remember:expr,
        background: $background:expr,
        success: $success:expr,
        error: $error:expr,
        warning: $warning:expr,
        merged: $merged:expr,
        warning_shimmer: $warning_shimmer:expr,
        diff_added: $diff_added:expr,
        diff_removed: $diff_removed:expr,
        diff_added_dimmed: $diff_added_dimmed:expr,
        diff_removed_dimmed: $diff_removed_dimmed:expr,
        diff_added_word: $diff_added_word:expr,
        diff_removed_word: $diff_removed_word:expr,
        red_for_subagents_only: $red_sub:expr,
        blue_for_subagents_only: $blue_sub:expr,
        green_for_subagents_only: $green_sub:expr,
        yellow_for_subagents_only: $yellow_sub:expr,
        purple_for_subagents_only: $purple_sub:expr,
        orange_for_subagents_only: $orange_sub:expr,
        pink_for_subagents_only: $pink_sub:expr,
        cyan_for_subagents_only: $cyan_sub:expr,
        professional_blue: $prof_blue:expr,
        chrome_yellow: $chrome_yellow:expr,
        clawd_body: $clawd_body:expr,
        clawd_background: $clawd_bg:expr,
        user_message_background: $user_msg_bg:expr,
        user_message_background_hover: $user_msg_bg_hover:expr,
        message_actions_background: $msg_actions_bg:expr,
        selection_bg: $selection_bg:expr,
        bash_message_background_color: $bash_msg_bg:expr,
        memory_background_color: $memory_bg:expr,
        rate_limit_fill: $rl_fill:expr,
        rate_limit_empty: $rl_empty:expr,
        fast_mode: $fast_mode:expr,
        fast_mode_shimmer: $fast_mode_shimmer:expr,
        brief_label_you: $brief_you:expr,
        brief_label_primary: $brief_primary:expr,
        rainbow_red: $rr:expr,
        rainbow_orange: $ro:expr,
        rainbow_yellow: $ry:expr,
        rainbow_green: $rg:expr,
        rainbow_blue: $rb:expr,
        rainbow_indigo: $ri:expr,
        rainbow_violet: $rv:expr,
        rainbow_red_shimmer: $rrs:expr,
        rainbow_orange_shimmer: $ros:expr,
        rainbow_yellow_shimmer: $rys:expr,
        rainbow_green_shimmer: $rgs:expr,
        rainbow_blue_shimmer: $rbs:expr,
        rainbow_indigo_shimmer: $ris:expr,
        rainbow_violet_shimmer: $rvs:expr,
    }) => {
        static $name: std::sync::LazyLock<Theme> = std::sync::LazyLock::new(|| Theme {
            auto_accept: $auto_accept.into(),
            bash_border: $bash_border.into(),
            primary: $primary.into(),
            primary_shimmer: $primary_shimmer.into(),
            blue_for_system_spinner: $pb_spinner.into(),
            blue_shimmer_for_system_spinner: $pb_shimmer_spinner.into(),
            permission: $permission.into(),
            permission_shimmer: $permission_shimmer.into(),
            plan_mode: $plan_mode.into(),
            ide: $ide.into(),
            prompt_border: $prompt_border.into(),
            prompt_border_shimmer: $prompt_border_shimmer.into(),
            text: $text.into(),
            inverse_text: $inverse_text.into(),
            inactive: $inactive.into(),
            inactive_shimmer: $inactive_shimmer.into(),
            subtle: $subtle.into(),
            suggestion: $suggestion.into(),
            remember: $remember.into(),
            background: $background.into(),
            success: $success.into(),
            error: $error.into(),
            warning: $warning.into(),
            merged: $merged.into(),
            warning_shimmer: $warning_shimmer.into(),
            diff_added: $diff_added.into(),
            diff_removed: $diff_removed.into(),
            diff_added_dimmed: $diff_added_dimmed.into(),
            diff_removed_dimmed: $diff_removed_dimmed.into(),
            diff_added_word: $diff_added_word.into(),
            diff_removed_word: $diff_removed_word.into(),
            red_for_subagents_only: $red_sub.into(),
            blue_for_subagents_only: $blue_sub.into(),
            green_for_subagents_only: $green_sub.into(),
            yellow_for_subagents_only: $yellow_sub.into(),
            purple_for_subagents_only: $purple_sub.into(),
            orange_for_subagents_only: $orange_sub.into(),
            pink_for_subagents_only: $pink_sub.into(),
            cyan_for_subagents_only: $cyan_sub.into(),
            professional_blue: $prof_blue.into(),
            chrome_yellow: $chrome_yellow.into(),
            clawd_body: $clawd_body.into(),
            clawd_background: $clawd_bg.into(),
            user_message_background: $user_msg_bg.into(),
            user_message_background_hover: $user_msg_bg_hover.into(),
            message_actions_background: $msg_actions_bg.into(),
            selection_bg: $selection_bg.into(),
            bash_message_background_color: $bash_msg_bg.into(),
            memory_background_color: $memory_bg.into(),
            rate_limit_fill: $rl_fill.into(),
            rate_limit_empty: $rl_empty.into(),
            fast_mode: $fast_mode.into(),
            fast_mode_shimmer: $fast_mode_shimmer.into(),
            brief_label_you: $brief_you.into(),
            brief_label_primary: $brief_primary.into(),
            rainbow_red: $rr.into(),
            rainbow_orange: $ro.into(),
            rainbow_yellow: $ry.into(),
            rainbow_green: $rg.into(),
            rainbow_blue: $rb.into(),
            rainbow_indigo: $ri.into(),
            rainbow_violet: $rv.into(),
            rainbow_red_shimmer: $rrs.into(),
            rainbow_orange_shimmer: $ros.into(),
            rainbow_yellow_shimmer: $rys.into(),
            rainbow_green_shimmer: $rgs.into(),
            rainbow_blue_shimmer: $rbs.into(),
            rainbow_indigo_shimmer: $ris.into(),
            rainbow_violet_shimmer: $rvs.into(),
        });
    };
}

// -- Light theme (explicit RGB) ------------------------------------------------

define_theme!(LIGHT_THEME, {
    auto_accept: "rgb(135,0,255)",
    bash_border: "rgb(255,0,135)",
    primary: "rgb(215,119,87)",
    primary_shimmer: "rgb(245,149,117)",
    blue_for_system_spinner: "rgb(87,105,247)",
    blue_shimmer_for_system_spinner: "rgb(117,135,255)",
    permission: "rgb(87,105,247)",
    permission_shimmer: "rgb(137,155,255)",
    plan_mode: "rgb(0,102,102)",
    ide: "rgb(71,130,200)",
    prompt_border: "rgb(153,153,153)",
    prompt_border_shimmer: "rgb(183,183,183)",
    text: "rgb(0,0,0)",
    inverse_text: "rgb(255,255,255)",
    inactive: "rgb(102,102,102)",
    inactive_shimmer: "rgb(142,142,142)",
    subtle: "rgb(175,175,175)",
    suggestion: "rgb(87,105,247)",
    remember: "rgb(0,0,255)",
    background: "rgb(0,153,153)",
    success: "rgb(44,122,57)",
    error: "rgb(171,43,63)",
    warning: "rgb(150,108,30)",
    merged: "rgb(135,0,255)",
    warning_shimmer: "rgb(200,158,80)",
    diff_added: "rgb(105,219,124)",
    diff_removed: "rgb(255,168,180)",
    diff_added_dimmed: "rgb(199,225,203)",
    diff_removed_dimmed: "rgb(253,210,216)",
    diff_added_word: "rgb(47,157,68)",
    diff_removed_word: "rgb(209,69,75)",
    red_for_subagents_only: "rgb(220,38,38)",
    blue_for_subagents_only: "rgb(37,99,235)",
    green_for_subagents_only: "rgb(22,163,74)",
    yellow_for_subagents_only: "rgb(202,138,4)",
    purple_for_subagents_only: "rgb(147,51,234)",
    orange_for_subagents_only: "rgb(234,88,12)",
    pink_for_subagents_only: "rgb(219,39,119)",
    cyan_for_subagents_only: "rgb(8,145,178)",
    professional_blue: "rgb(106,155,204)",
    chrome_yellow: "rgb(251,188,4)",
    clawd_body: "rgb(215,119,87)",
    clawd_background: "rgb(0,0,0)",
    user_message_background: "rgb(240,240,240)",
    user_message_background_hover: "rgb(252,252,252)",
    message_actions_background: "rgb(232,236,244)",
    selection_bg: "rgb(180,213,255)",
    bash_message_background_color: "rgb(250,245,250)",
    memory_background_color: "rgb(230,245,250)",
    rate_limit_fill: "rgb(87,105,247)",
    rate_limit_empty: "rgb(39,47,111)",
    fast_mode: "rgb(255,106,0)",
    fast_mode_shimmer: "rgb(255,150,50)",
    brief_label_you: "rgb(37,99,235)",
    brief_label_primary: "rgb(215,119,87)",
    rainbow_red: "rgb(235,95,87)",
    rainbow_orange: "rgb(245,139,87)",
    rainbow_yellow: "rgb(250,195,95)",
    rainbow_green: "rgb(145,200,130)",
    rainbow_blue: "rgb(130,170,220)",
    rainbow_indigo: "rgb(155,130,200)",
    rainbow_violet: "rgb(200,130,180)",
    rainbow_red_shimmer: "rgb(250,155,147)",
    rainbow_orange_shimmer: "rgb(255,185,137)",
    rainbow_yellow_shimmer: "rgb(255,225,155)",
    rainbow_green_shimmer: "rgb(185,230,180)",
    rainbow_blue_shimmer: "rgb(180,205,240)",
    rainbow_indigo_shimmer: "rgb(195,180,230)",
    rainbow_violet_shimmer: "rgb(230,180,210)",
});

// -- Dark theme (explicit RGB) -------------------------------------------------

define_theme!(DARK_THEME, {
    auto_accept: "rgb(175,135,255)",
    bash_border: "rgb(253,93,177)",
    primary: "rgb(215,119,87)",
    primary_shimmer: "rgb(235,159,127)",
    blue_for_system_spinner: "rgb(147,165,255)",
    blue_shimmer_for_system_spinner: "rgb(177,195,255)",
    permission: "rgb(177,185,249)",
    permission_shimmer: "rgb(207,215,255)",
    plan_mode: "rgb(72,150,140)",
    ide: "rgb(71,130,200)",
    prompt_border: "rgb(136,136,136)",
    prompt_border_shimmer: "rgb(166,166,166)",
    text: "rgb(255,255,255)",
    inverse_text: "rgb(0,0,0)",
    inactive: "rgb(153,153,153)",
    inactive_shimmer: "rgb(193,193,193)",
    subtle: "rgb(80,80,80)",
    suggestion: "rgb(177,185,249)",
    remember: "rgb(177,185,249)",
    background: "rgb(0,204,204)",
    success: "rgb(78,186,101)",
    error: "rgb(255,107,128)",
    warning: "rgb(255,193,7)",
    merged: "rgb(175,135,255)",
    warning_shimmer: "rgb(255,223,57)",
    diff_added: "rgb(34,92,43)",
    diff_removed: "rgb(122,41,54)",
    diff_added_dimmed: "rgb(71,88,74)",
    diff_removed_dimmed: "rgb(105,72,77)",
    diff_added_word: "rgb(56,166,96)",
    diff_removed_word: "rgb(179,89,107)",
    red_for_subagents_only: "rgb(220,38,38)",
    blue_for_subagents_only: "rgb(37,99,235)",
    green_for_subagents_only: "rgb(22,163,74)",
    yellow_for_subagents_only: "rgb(202,138,4)",
    purple_for_subagents_only: "rgb(147,51,234)",
    orange_for_subagents_only: "rgb(234,88,12)",
    pink_for_subagents_only: "rgb(219,39,119)",
    cyan_for_subagents_only: "rgb(8,145,178)",
    professional_blue: "rgb(106,155,204)",
    chrome_yellow: "rgb(251,188,4)",
    clawd_body: "rgb(215,119,87)",
    clawd_background: "rgb(0,0,0)",
    user_message_background: "rgb(55,55,55)",
    user_message_background_hover: "rgb(70,70,70)",
    message_actions_background: "rgb(44,50,62)",
    selection_bg: "rgb(38,79,120)",
    bash_message_background_color: "rgb(65,60,65)",
    memory_background_color: "rgb(55,65,70)",
    rate_limit_fill: "rgb(177,185,249)",
    rate_limit_empty: "rgb(80,83,112)",
    fast_mode: "rgb(255,120,20)",
    fast_mode_shimmer: "rgb(255,165,70)",
    brief_label_you: "rgb(122,180,232)",
    brief_label_primary: "rgb(215,119,87)",
    rainbow_red: "rgb(235,95,87)",
    rainbow_orange: "rgb(245,139,87)",
    rainbow_yellow: "rgb(250,195,95)",
    rainbow_green: "rgb(145,200,130)",
    rainbow_blue: "rgb(130,170,220)",
    rainbow_indigo: "rgb(155,130,200)",
    rainbow_violet: "rgb(200,130,180)",
    rainbow_red_shimmer: "rgb(250,155,147)",
    rainbow_orange_shimmer: "rgb(255,185,137)",
    rainbow_yellow_shimmer: "rgb(255,225,155)",
    rainbow_green_shimmer: "rgb(185,230,180)",
    rainbow_blue_shimmer: "rgb(180,205,240)",
    rainbow_indigo_shimmer: "rgb(195,180,230)",
    rainbow_violet_shimmer: "rgb(230,180,210)",
});

// -- Light ANSI theme ----------------------------------------------------------

define_theme!(LIGHT_ANSI_THEME, {
    auto_accept: "ansi:magenta",
    bash_border: "ansi:magenta",
    primary: "ansi:redBright",
    primary_shimmer: "ansi:yellowBright",
    blue_for_system_spinner: "ansi:blue",
    blue_shimmer_for_system_spinner: "ansi:blueBright",
    permission: "ansi:blue",
    permission_shimmer: "ansi:blueBright",
    plan_mode: "ansi:cyan",
    ide: "ansi:blueBright",
    prompt_border: "ansi:white",
    prompt_border_shimmer: "ansi:whiteBright",
    text: "ansi:black",
    inverse_text: "ansi:white",
    inactive: "ansi:blackBright",
    inactive_shimmer: "ansi:white",
    subtle: "ansi:blackBright",
    suggestion: "ansi:blue",
    remember: "ansi:blue",
    background: "ansi:cyan",
    success: "ansi:green",
    error: "ansi:red",
    warning: "ansi:yellow",
    merged: "ansi:magenta",
    warning_shimmer: "ansi:yellowBright",
    diff_added: "ansi:green",
    diff_removed: "ansi:red",
    diff_added_dimmed: "ansi:green",
    diff_removed_dimmed: "ansi:red",
    diff_added_word: "ansi:greenBright",
    diff_removed_word: "ansi:redBright",
    red_for_subagents_only: "ansi:red",
    blue_for_subagents_only: "ansi:blue",
    green_for_subagents_only: "ansi:green",
    yellow_for_subagents_only: "ansi:yellow",
    purple_for_subagents_only: "ansi:magenta",
    orange_for_subagents_only: "ansi:redBright",
    pink_for_subagents_only: "ansi:magentaBright",
    cyan_for_subagents_only: "ansi:cyan",
    professional_blue: "ansi:blueBright",
    chrome_yellow: "ansi:yellow",
    clawd_body: "ansi:redBright",
    clawd_background: "ansi:black",
    user_message_background: "ansi:white",
    user_message_background_hover: "ansi:whiteBright",
    message_actions_background: "ansi:white",
    selection_bg: "ansi:cyan",
    bash_message_background_color: "ansi:whiteBright",
    memory_background_color: "ansi:white",
    rate_limit_fill: "ansi:yellow",
    rate_limit_empty: "ansi:black",
    fast_mode: "ansi:red",
    fast_mode_shimmer: "ansi:redBright",
    brief_label_you: "ansi:blue",
    brief_label_primary: "ansi:redBright",
    rainbow_red: "ansi:red",
    rainbow_orange: "ansi:redBright",
    rainbow_yellow: "ansi:yellow",
    rainbow_green: "ansi:green",
    rainbow_blue: "ansi:cyan",
    rainbow_indigo: "ansi:blue",
    rainbow_violet: "ansi:magenta",
    rainbow_red_shimmer: "ansi:redBright",
    rainbow_orange_shimmer: "ansi:yellow",
    rainbow_yellow_shimmer: "ansi:yellowBright",
    rainbow_green_shimmer: "ansi:greenBright",
    rainbow_blue_shimmer: "ansi:cyanBright",
    rainbow_indigo_shimmer: "ansi:blueBright",
    rainbow_violet_shimmer: "ansi:magentaBright",
});

// -- Dark ANSI theme -----------------------------------------------------------

define_theme!(DARK_ANSI_THEME, {
    auto_accept: "ansi:magentaBright",
    bash_border: "ansi:magentaBright",
    primary: "ansi:redBright",
    primary_shimmer: "ansi:yellowBright",
    blue_for_system_spinner: "ansi:blueBright",
    blue_shimmer_for_system_spinner: "ansi:blueBright",
    permission: "ansi:blueBright",
    permission_shimmer: "ansi:blueBright",
    plan_mode: "ansi:cyanBright",
    ide: "ansi:blue",
    prompt_border: "ansi:white",
    prompt_border_shimmer: "ansi:whiteBright",
    text: "ansi:whiteBright",
    inverse_text: "ansi:black",
    inactive: "ansi:white",
    inactive_shimmer: "ansi:whiteBright",
    subtle: "ansi:white",
    suggestion: "ansi:blueBright",
    remember: "ansi:blueBright",
    background: "ansi:cyanBright",
    success: "ansi:greenBright",
    error: "ansi:redBright",
    warning: "ansi:yellowBright",
    merged: "ansi:magentaBright",
    warning_shimmer: "ansi:yellowBright",
    diff_added: "ansi:green",
    diff_removed: "ansi:red",
    diff_added_dimmed: "ansi:green",
    diff_removed_dimmed: "ansi:red",
    diff_added_word: "ansi:greenBright",
    diff_removed_word: "ansi:redBright",
    red_for_subagents_only: "ansi:redBright",
    blue_for_subagents_only: "ansi:blueBright",
    green_for_subagents_only: "ansi:greenBright",
    yellow_for_subagents_only: "ansi:yellowBright",
    purple_for_subagents_only: "ansi:magentaBright",
    orange_for_subagents_only: "ansi:redBright",
    pink_for_subagents_only: "ansi:magentaBright",
    cyan_for_subagents_only: "ansi:cyanBright",
    professional_blue: "rgb(106,155,204)",
    chrome_yellow: "ansi:yellowBright",
    clawd_body: "ansi:redBright",
    clawd_background: "ansi:black",
    user_message_background: "ansi:blackBright",
    user_message_background_hover: "ansi:white",
    message_actions_background: "ansi:blackBright",
    selection_bg: "ansi:blue",
    bash_message_background_color: "ansi:black",
    memory_background_color: "ansi:blackBright",
    rate_limit_fill: "ansi:yellow",
    rate_limit_empty: "ansi:white",
    fast_mode: "ansi:redBright",
    fast_mode_shimmer: "ansi:redBright",
    brief_label_you: "ansi:blueBright",
    brief_label_primary: "ansi:redBright",
    rainbow_red: "ansi:red",
    rainbow_orange: "ansi:redBright",
    rainbow_yellow: "ansi:yellow",
    rainbow_green: "ansi:green",
    rainbow_blue: "ansi:cyan",
    rainbow_indigo: "ansi:blue",
    rainbow_violet: "ansi:magenta",
    rainbow_red_shimmer: "ansi:redBright",
    rainbow_orange_shimmer: "ansi:yellow",
    rainbow_yellow_shimmer: "ansi:yellowBright",
    rainbow_green_shimmer: "ansi:greenBright",
    rainbow_blue_shimmer: "ansi:cyanBright",
    rainbow_indigo_shimmer: "ansi:blueBright",
    rainbow_violet_shimmer: "ansi:magentaBright",
});

// -- Light daltonized theme (color-blind friendly, explicit RGB) ---------------

define_theme!(LIGHT_DALTONIZED_THEME, {
    auto_accept: "rgb(135,0,255)",
    bash_border: "rgb(0,102,204)",
    primary: "rgb(255,153,51)",
    primary_shimmer: "rgb(255,183,101)",
    blue_for_system_spinner: "rgb(51,102,255)",
    blue_shimmer_for_system_spinner: "rgb(101,152,255)",
    permission: "rgb(51,102,255)",
    permission_shimmer: "rgb(101,152,255)",
    plan_mode: "rgb(51,102,102)",
    ide: "rgb(71,130,200)",
    prompt_border: "rgb(153,153,153)",
    prompt_border_shimmer: "rgb(183,183,183)",
    text: "rgb(0,0,0)",
    inverse_text: "rgb(255,255,255)",
    inactive: "rgb(102,102,102)",
    inactive_shimmer: "rgb(142,142,142)",
    subtle: "rgb(175,175,175)",
    suggestion: "rgb(51,102,255)",
    remember: "rgb(51,102,255)",
    background: "rgb(0,153,153)",
    success: "rgb(0,102,153)",
    error: "rgb(204,0,0)",
    warning: "rgb(255,153,0)",
    merged: "rgb(135,0,255)",
    warning_shimmer: "rgb(255,183,50)",
    diff_added: "rgb(153,204,255)",
    diff_removed: "rgb(255,204,204)",
    diff_added_dimmed: "rgb(209,231,253)",
    diff_removed_dimmed: "rgb(255,233,233)",
    diff_added_word: "rgb(51,102,204)",
    diff_removed_word: "rgb(153,51,51)",
    red_for_subagents_only: "rgb(204,0,0)",
    blue_for_subagents_only: "rgb(0,102,204)",
    green_for_subagents_only: "rgb(0,204,0)",
    yellow_for_subagents_only: "rgb(255,204,0)",
    purple_for_subagents_only: "rgb(128,0,128)",
    orange_for_subagents_only: "rgb(255,128,0)",
    pink_for_subagents_only: "rgb(255,102,178)",
    cyan_for_subagents_only: "rgb(0,178,178)",
    professional_blue: "rgb(106,155,204)",
    chrome_yellow: "rgb(251,188,4)",
    clawd_body: "rgb(215,119,87)",
    clawd_background: "rgb(0,0,0)",
    user_message_background: "rgb(220,220,220)",
    user_message_background_hover: "rgb(232,232,232)",
    message_actions_background: "rgb(210,216,226)",
    selection_bg: "rgb(180,213,255)",
    bash_message_background_color: "rgb(250,245,250)",
    memory_background_color: "rgb(230,245,250)",
    rate_limit_fill: "rgb(51,102,255)",
    rate_limit_empty: "rgb(23,46,114)",
    fast_mode: "rgb(255,106,0)",
    fast_mode_shimmer: "rgb(255,150,50)",
    brief_label_you: "rgb(37,99,235)",
    brief_label_primary: "rgb(255,153,51)",
    rainbow_red: "rgb(235,95,87)",
    rainbow_orange: "rgb(245,139,87)",
    rainbow_yellow: "rgb(250,195,95)",
    rainbow_green: "rgb(145,200,130)",
    rainbow_blue: "rgb(130,170,220)",
    rainbow_indigo: "rgb(155,130,200)",
    rainbow_violet: "rgb(200,130,180)",
    rainbow_red_shimmer: "rgb(250,155,147)",
    rainbow_orange_shimmer: "rgb(255,185,137)",
    rainbow_yellow_shimmer: "rgb(255,225,155)",
    rainbow_green_shimmer: "rgb(185,230,180)",
    rainbow_blue_shimmer: "rgb(180,205,240)",
    rainbow_indigo_shimmer: "rgb(195,180,230)",
    rainbow_violet_shimmer: "rgb(230,180,210)",
});

// -- Dark daltonized theme (color-blind friendly, explicit RGB) ----------------

define_theme!(DARK_DALTONIZED_THEME, {
    auto_accept: "rgb(175,135,255)",
    bash_border: "rgb(51,153,255)",
    primary: "rgb(255,153,51)",
    primary_shimmer: "rgb(255,183,101)",
    blue_for_system_spinner: "rgb(153,204,255)",
    blue_shimmer_for_system_spinner: "rgb(183,224,255)",
    permission: "rgb(153,204,255)",
    permission_shimmer: "rgb(183,224,255)",
    plan_mode: "rgb(102,153,153)",
    ide: "rgb(71,130,200)",
    prompt_border: "rgb(136,136,136)",
    prompt_border_shimmer: "rgb(166,166,166)",
    text: "rgb(255,255,255)",
    inverse_text: "rgb(0,0,0)",
    inactive: "rgb(153,153,153)",
    inactive_shimmer: "rgb(193,193,193)",
    subtle: "rgb(80,80,80)",
    suggestion: "rgb(153,204,255)",
    remember: "rgb(153,204,255)",
    background: "rgb(0,204,204)",
    success: "rgb(51,153,255)",
    error: "rgb(255,102,102)",
    warning: "rgb(255,204,0)",
    merged: "rgb(175,135,255)",
    warning_shimmer: "rgb(255,234,50)",
    diff_added: "rgb(0,68,102)",
    diff_removed: "rgb(102,0,0)",
    diff_added_dimmed: "rgb(62,81,91)",
    diff_removed_dimmed: "rgb(62,44,44)",
    diff_added_word: "rgb(0,119,179)",
    diff_removed_word: "rgb(179,0,0)",
    red_for_subagents_only: "rgb(255,102,102)",
    blue_for_subagents_only: "rgb(102,178,255)",
    green_for_subagents_only: "rgb(102,255,102)",
    yellow_for_subagents_only: "rgb(255,255,102)",
    purple_for_subagents_only: "rgb(178,102,255)",
    orange_for_subagents_only: "rgb(255,178,102)",
    pink_for_subagents_only: "rgb(255,153,204)",
    cyan_for_subagents_only: "rgb(102,204,204)",
    professional_blue: "rgb(106,155,204)",
    chrome_yellow: "rgb(251,188,4)",
    clawd_body: "rgb(215,119,87)",
    clawd_background: "rgb(0,0,0)",
    user_message_background: "rgb(55,55,55)",
    user_message_background_hover: "rgb(70,70,70)",
    message_actions_background: "rgb(44,50,62)",
    selection_bg: "rgb(38,79,120)",
    bash_message_background_color: "rgb(65,60,65)",
    memory_background_color: "rgb(55,65,70)",
    rate_limit_fill: "rgb(153,204,255)",
    rate_limit_empty: "rgb(69,92,115)",
    fast_mode: "rgb(255,120,20)",
    fast_mode_shimmer: "rgb(255,165,70)",
    brief_label_you: "rgb(122,180,232)",
    brief_label_primary: "rgb(255,153,51)",
    rainbow_red: "rgb(235,95,87)",
    rainbow_orange: "rgb(245,139,87)",
    rainbow_yellow: "rgb(250,195,95)",
    rainbow_green: "rgb(145,200,130)",
    rainbow_blue: "rgb(130,170,220)",
    rainbow_indigo: "rgb(155,130,200)",
    rainbow_violet: "rgb(200,130,180)",
    rainbow_red_shimmer: "rgb(250,155,147)",
    rainbow_orange_shimmer: "rgb(255,185,137)",
    rainbow_yellow_shimmer: "rgb(255,225,155)",
    rainbow_green_shimmer: "rgb(185,230,180)",
    rainbow_blue_shimmer: "rgb(180,205,240)",
    rainbow_indigo_shimmer: "rgb(195,180,230)",
    rainbow_violet_shimmer: "rgb(230,180,210)",
});

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_themes_accessible() {
        for name in THEME_NAMES {
            let theme = get_theme(*name);
            assert!(!theme.text.is_empty(), "theme {:?} has empty text", name);
            assert!(!theme.primary.is_empty(), "theme {:?} has empty primary", name);
        }
    }

    #[test]
    fn theme_setting_resolve_auto_dark() {
        let resolved = ThemeSetting::Auto.resolve(Some(true));
        assert_eq!(resolved, ThemeName::Dark);
    }

    #[test]
    fn theme_setting_resolve_auto_light() {
        let resolved = ThemeSetting::Auto.resolve(Some(false));
        assert_eq!(resolved, ThemeName::Light);
    }

    #[test]
    fn theme_setting_resolve_auto_default() {
        let resolved = ThemeSetting::Auto.resolve(None);
        assert_eq!(resolved, ThemeName::Dark);
    }

    #[test]
    fn theme_setting_resolve_explicit() {
        assert_eq!(
            ThemeSetting::LightDaltonized.resolve(Some(true)),
            ThemeName::LightDaltonized
        );
    }

    #[test]
    fn dark_theme_has_rgb_colors() {
        let theme = get_theme(ThemeName::Dark);
        assert!(theme.text.starts_with("rgb("));
        assert!(theme.primary.starts_with("rgb("));
    }

    #[test]
    fn ansi_theme_has_ansi_colors() {
        let theme = get_theme(ThemeName::DarkAnsi);
        assert!(theme.text.starts_with("ansi:"));
        assert!(theme.primary.starts_with("ansi:"));
    }

    #[test]
    fn theme_serde_roundtrip() {
        let name = ThemeName::Light;
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"light\"");
        let parsed: ThemeName = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, name);
    }

    #[test]
    fn theme_setting_serde_auto() {
        let setting = ThemeSetting::Auto;
        let json = serde_json::to_string(&setting).unwrap();
        assert_eq!(json, "\"auto\"");
        let parsed: ThemeSetting = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, setting);
    }
}
