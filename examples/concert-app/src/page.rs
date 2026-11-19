//! Presentation for the browser's public program and personal reservation desk.

use std::fmt::Write;

use gdd_concert::{Catalog, History, State};
use uuid::Uuid;

use crate::identity::Profile;

pub struct Desk {
    pub profile: Profile,
    pub page_id: Uuid,
    pub history: History,
}

pub const SIGN_IN: &str = include_str!("page/sign-in.html");

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn hidden(name: &str, value: &str) -> String {
    format!(
        r#"<input type="hidden" name="{name}" value="{}">"#,
        escape(value)
    )
}

fn view_field(profile: Profile) -> String {
    hidden("view", &profile.view.to_string())
}

pub fn document(
    profile: Option<Profile>,
    home: &str,
    notice: &str,
    error: &str,
    content: &str,
) -> String {
    let (identity, logout) = match profile {
        Some(profile) => (
            format!("Signed in as {}", escape(profile.name)),
            format!(
                r#"<form method="post" action="/forms/logout">{}<button id="sign-out" class="button-secondary">Sign out</button></form>"#,
                view_field(profile),
            ),
        ),
        None => ("You're signed out".to_owned(), String::new()),
    };
    format!(
        include_str!("page/document.html"),
        home = escape(home),
        notice = escape(notice),
        error = escape(error),
        error_hidden = if error.is_empty() { " hidden" } else { "" },
        identity = identity,
        logout = logout,
        content = content
    )
}

pub fn back_to_program(home: &str) -> String {
    format!(
        r#"<p><a href="{}">Return to the concert program</a></p>"#,
        escape(home)
    )
}

pub fn reserve_form(
    profile: Profile,
    concert_id: &str,
    key: &str,
    label: &str,
    disabled: bool,
) -> String {
    format!(
        r#"<form method="post" action="/forms/reserve">{}{}{}<button{}>{}</button></form>"#,
        view_field(profile),
        hidden("concert_id", concert_id),
        hidden("request_key", key),
        if disabled { " disabled" } else { "" },
        escape(label)
    )
}

fn refresh(home: &str, section: &str, heading: &str, name: &str) -> String {
    format!(
        r#"<form method="get" action="{}#{heading}-heading"><button class="button-secondary" id="refresh-{section}">{name}</button></form>"#,
        escape(home)
    )
}

pub fn program(home: &str, catalog: &Catalog, desk: Option<&Desk>) -> String {
    let mut content = String::new();
    if desk.is_none() {
        content.push_str(SIGN_IN);
    }
    let capped = desk.is_some_and(|desk| {
        desk.history
            .active_limit
            .is_some_and(|limit| desk.history.active_count as i64 >= limit)
    });
    write!(content, r#"<section class="program" aria-labelledby="program-heading"><div class="section-heading"><div>
        <p class="eyebrow">Choose your evening</p><h2 id="program-heading" tabindex="-1">Upcoming concerts</h2>
        <p id="catalog-summary" class="section-description">Availability is current as of this page load.</p></div>
        {}</div><div class="concert-grid" id="concerts">"#, refresh(home, "concerts", "program", "Refresh concerts")).unwrap();
    for (index, concert) in catalog.concerts.iter().enumerate() {
        let sold_out = concert.available == 0;
        let availability = if sold_out {
            format!("Sold out · {} seats", concert.capacity)
        } else {
            format!(
                "{} of {} seats available",
                concert.available, concert.capacity
            )
        };
        write!(content, r#"<article class="concert-card" data-concert-id="{}" aria-labelledby="concert-{index}">
            <div class="card-program-line"><time datetime="{}">{}</time></div><h3 id="concert-{index}">{}</h3><p class="concert-venue">{}</p>
            <p class="availability{}">{availability}</p>"#,
            escape(&concert.id), escape(&concert.date), escape(&concert.date), escape(&concert.title), escape(&concert.venue),
            if sold_out { " sold-out" } else { "" }).unwrap();
        if let Some(desk) = desk {
            // A reloaded or restored page still carries the original key. A
            // successful mutation redirects to a fresh page for the next seat.
            let key = format!("{}:{}", desk.page_id, concert.id);
            let label = if sold_out {
                "Sold out"
            } else if capped {
                "Allowance full"
            } else {
                "Reserve one seat"
            };
            content.push_str(&reserve_form(
                desk.profile,
                &concert.id,
                &key,
                label,
                sold_out || capped,
            ));
        } else {
            content.push_str(if sold_out {
                "<button disabled>Sold out</button>"
            } else {
                "<button disabled>Sign in to reserve</button>"
            });
        }
        content.push_str("</article>");
    }
    if catalog.concerts.is_empty() {
        content.push_str(r#"<p class="empty-state">No concerts are currently in the program.</p>"#);
    }
    content.push_str("</div></section>");
    if let Some(desk) = desk {
        history_section(
            &mut content,
            home,
            desk.profile,
            catalog,
            &desk.history,
            capped,
        );
    }
    content
}

fn history_section(
    content: &mut String,
    home: &str,
    profile: Profile,
    catalog: &Catalog,
    history: &History,
    capped: bool,
) {
    let summary = match history.active_limit {
        Some(limit) => format!("{} of {limit} active reservations.", history.active_count),
        None => format!("{} active reservations.", history.active_count),
    };
    write!(content, r#"<section id="reservations-section" class="reservations-section" aria-labelledby="reservations-heading">
        <div class="section-heading"><div><p class="eyebrow">Your reservation desk</p>
        <h2 id="reservations-heading" tabindex="-1">Your reservations</h2>
        <p class="section-description" id="reservation-summary">{summary}</p></div>{}</div>"#,
        refresh(home, "reservations", "reservations", "Refresh reservations")).unwrap();
    if capped {
        content.push_str(r#"<p id="allowance-message" class="allowance-message">Your current allowance is full. Cancel a reservation to make room for another concert.</p>"#);
    }
    content.push_str(r#"<div id="reservation-history">"#);
    if history.reservations.is_empty() {
        content.push_str(
            r#"<p class="empty-state">Choose a concert to make your first reservation.</p>"#,
        );
    } else {
        content.push_str(r#"<ul class="reservation-list">"#);
        for (index, reservation) in history.reservations.iter().enumerate() {
            let title = catalog
                .concerts
                .iter()
                .find(|c| c.id == reservation.concert_id)
                .map_or(&reservation.concert_id, |c| &c.title);
            let (state, label) = match reservation.state {
                State::Reserved => ("reserved", "Reserved"),
                State::Cancelled => ("cancelled", "Cancelled"),
            };
            write!(content, r#"<li class="reservation-item" data-reservation-id="{id}">
                <div class="reservation-detail"><h3 id="reserved-title-{index}">{title}</h3><p id="reference-{index}" class="reservation-reference">Reference {id}</p></div>
                <div class="reservation-actions"><span class="reservation-state state-{state}">{label}</span>"#,
                id=escape(&reservation.id), title=escape(title)).unwrap();
            if reservation.state == State::Reserved {
                write!(content, r#"<form method="post" action="/forms/cancel">{}{}<button class="button-secondary" aria-describedby="reserved-title-{index} reference-{index}">Cancel reservation</button></form>"#,
                    view_field(profile), hidden("reservation_id", &reservation.id)).unwrap();
            }
            content.push_str("</div></li>");
        }
        content.push_str("</ul>");
    }
    content.push_str("</div></section>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdd_concert::Concert;

    #[test]
    fn program_preserves_punctuation_in_titles_venues_and_identifiers() {
        let catalog = Catalog {
            concerts: vec![Concert {
                id: "folk-\"duet\"".into(),
                title: "Søren & Ida <acoustic>".into(),
                venue: "The Fisherman's Club".into(),
                date: "2026-11-21".into(),
                capacity: 12,
                available: 5,
            }],
        };
        let html = program("/program", &catalog, None);
        assert!(html.contains(r#"data-concert-id="folk-&quot;duet&quot;""#));
        assert!(html.contains("Søren &amp; Ida &lt;acoustic&gt;"));
        assert!(html.contains("The Fisherman&#39;s Club"));
    }
}
