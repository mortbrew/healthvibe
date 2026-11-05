//! Apple Fitness awards the user marks as earned.
//!
//! Apple's earned-award list is not in the Health export and healthvibe does
//! not compute awards or streaks from data. This is only the catalog of
//! award kinds a user can tick off (stored in `apple_awards`, with an
//! optional date they enter).

/// `(key, name, description)` of awards a user can mark as earned.
pub const APPLE_ONLY: &[(&str, &str, &str)] = &[
    (
        "monthly_challenge",
        "Monthly Challenge",
        "A personal challenge Apple sets each month based on your history.",
    ),
    (
        "limited_edition",
        "Limited Edition Awards",
        "Special awards for completing a workout or goal on a particular day or event.",
    ),
    (
        "special_day",
        "Holiday & Observance Awards",
        "Awards tied to specific days, such as national holidays or awareness days.",
    ),
    (
        "competition_win",
        "Competition Win",
        "Win a 7-day activity competition with a friend.",
    ),
    (
        "competition_done",
        "Competition Completed",
        "Finish a 7-day activity competition with a friend.",
    ),
    (
        "sharing",
        "Activity Sharing Awards",
        "Awards earned through sharing activity with friends.",
    ),
    (
        "service_workouts",
        "Subscription Workout Awards",
        "Milestones for workouts done in Apple's subscription fitness service.",
    ),
];
