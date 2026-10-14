"use strict";

const ui = Object.fromEntries([
  "session-status", "sign-out", "sign-in-panel", "sign-in-form", "username", "password", "sign-in",
  "status-message", "error-message", "concerts", "catalog-summary", "refresh-concerts",
  "reservations-section", "reservation-summary", "reservation-history", "allowance-message",
  "refresh-reservations", "program-heading", "reservations-heading",
].map(id => [id, document.getElementById(id)]));

let principal = null;
let identityEpoch = 0;
let concerts = [];
let reservations = [];
let activeCount = 0;
let activeLimit = null;
let initializing = true;
let signingIn = false;
let signingOut = false;
let signOutUncertain = false;
let signOutExpected = null;
let catalogLoading = true;
let historyLoading = false;
let catalogFailed = false;
let historyFailed = false;
let catalogRequest = 0;
let historyRequest = 0;

// A key belongs to one principal and concert attempt. Uncertain outcomes retain it.
const reserveKeys = new Map();
const reserveBusy = new Set();
const cancelBusy = new Set();
const concertButtons = new Map();
const concertHeadings = new Map();
const reservationHeadings = new Map();

class ApiError extends Error {
  constructor(code, message, status) {
    super(message);
    this.code = code;
    this.status = status;
  }
}

async function request(path, body, expectedPrincipal) {
  const expected = expectedPrincipal ?? (path.startsWith("/api/reservations") ? principal?.id : undefined);
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 12000);
  try {
    const response = await fetch(path, {
      method: body === undefined ? "GET" : "POST",
      credentials: "same-origin",
      headers: { Accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }),
        ...(expected ? { "X-Concert-Principal": expected } : {}),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      signal: controller.signal,
    });
    const data = await response.json();
    if (!response.ok) {
      if (!data.error || typeof data.error.code !== "string") throw new Error("Unconfirmed response.");
      throw new ApiError(data.error.code, data.error.message || "The request could not be completed.", response.status);
    }
    return data;
  } finally {
    window.clearTimeout(timeout);
  }
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function status(message) {
  ui["status-message"].textContent = message;
}

function error(message) {
  ui["error-message"].textContent = message;
  ui["error-message"].hidden = !message;
}

function errorText(failure) {
  const messages = {
    InvalidCredentials: "That username or password isn't right. Please try again.",
    InvalidInput: "Please check the information and try again.",
    SoldOut: "The last seat has been taken. Choose another concert or refresh availability.",
    ConcertNotFound: "This concert is no longer available. Refresh the program to choose another.",
    ReservationNotFound: "This reservation is not available in your account. Refresh your reservations.",
    IdempotencyConflict: "This request belongs to a different concert. Refresh the page before making another reservation.",
    ActiveReservationLimitReached: "Your active reservation allowance is full. Cancel a reservation to make room for another.",
    PersistenceUnavailable: "The reservation desk is temporarily unavailable. No changes were saved. Please try again.",
    Unauthenticated: "Your session has ended. Please sign in again.",
  };
  return messages[failure.code] || "We couldn't complete the request. Please try again.";
}

function atLimit() {
  return activeLimit !== null && activeCount >= activeLimit;
}

function setIdentity(next) {
  identityEpoch += 1;
  principal = next;
  reservations = [];
  activeCount = 0;
  activeLimit = null;
  catalogLoading = false;
  catalogRequest += 1;
  historyRequest += 1;
  historyLoading = false;
  historyFailed = false;
  reserveKeys.clear();
  reserveBusy.clear();
  cancelBusy.clear();
  ui.username.value = "";
  ui.password.value = "";
  render();
  return identityEpoch;
}

function endExpiredSession(epoch) {
  if (epoch !== identityEpoch) return;
  setIdentity(null);
  status("Sign in to continue with your reservations.");
  error("Your session has ended. Please sign in again.");
}

function renderSession() {
  ui["session-status"].textContent = initializing ? "Checking your session…" :
    signingIn ? "Signing in…" : signingOut ? "Signing out…" :
    signOutUncertain ? "Sign-out not yet confirmed" :
    principal ? `Signed in as ${principal.name}` : "You're signed out";
  ui["sign-out"].hidden = !principal && !signingOut && !signOutUncertain;
  ui["sign-out"].disabled = signingOut || signingIn;
  ui["sign-out"].textContent = signingOut ? "Signing out…" : signOutUncertain ? "Retry sign out" : "Sign out";
  ui["sign-in-panel"].hidden = Boolean(principal);
  for (const id of ["username", "password", "sign-in"]) {
    ui[id].disabled = initializing || signingIn || signingOut || signOutUncertain;
  }
  ui["sign-in"].textContent = signingIn ? "Signing in…" : "Sign in";
}

function dateLabel(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return value;
  const date = new Date(`${value}T12:00:00Z`);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("en", { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(date);
}

function renderConcerts() {
  ui.concerts.setAttribute("aria-busy", String(catalogLoading));
  ui["refresh-concerts"].disabled = catalogLoading;
  ui["refresh-concerts"].textContent = catalogLoading ? "Refreshing…" : "Refresh concerts";
  ui["catalog-summary"].textContent = catalogLoading ? "Checking seat availability." :
    catalogFailed ? "Availability couldn't be refreshed. Please try again." :
    "Availability is current when checked; your seat is confirmed when you reserve.";
  concertButtons.clear();
  concertHeadings.clear();
  ui.concerts.replaceChildren();
  if (!concerts.length) {
    ui.concerts.append(element("p", "empty-state", catalogLoading ? "Loading concerts…" :
      catalogFailed ? "The program couldn't be loaded. Use Refresh concerts to try again." :
      "There are no concerts in the program yet. Check back another time."));
    return;
  }
  concerts.forEach((concert, index) => {
    const card = element("article", "concert-card");
    const line = element("div", "card-program-line");
    const number = element("span", "card-number", String(index + 1).padStart(2, "0"));
    number.setAttribute("aria-hidden", "true");
    line.append(element("span", "", dateLabel(concert.date)), number);
    const heading = element("h3", "", concert.title);
    heading.tabIndex = -1;
    concertHeadings.set(concert.id, heading);
    card.append(line, heading, element("p", "concert-venue", concert.venue));
    const availability = element("p", `availability${concert.available === 0 ? " sold-out" : ""}`,
      concert.available === 0 ? `Sold out · ${concert.capacity} seats` :
        `${concert.available} of ${concert.capacity} seats available`);
    const retry = reserveKeys.has(concert.id);
    const pending = reserveBusy.has(concert.id);
    const button = element("button", "", pending ? "Reserving…" : retry ? "Retry reservation" :
      concert.available === 0 ? "Sold out" : !principal ? "Sign in to reserve" :
        atLimit() ? "Allowance full" : "Reserve one seat");
    button.type = "button";
    button.dataset.concertId = concert.id;
    button.setAttribute("aria-label", `${button.textContent} for ${concert.title}`);
    // A retry can return an existing success even when capacity or allowance is full.
    button.disabled = !principal || pending || initializing || signingIn || signingOut ||
      (!retry && (concert.available === 0 || atLimit()));
    button.addEventListener("click", () => reserve(concert.id));
    concertButtons.set(concert.id, button);
    card.append(availability, button);
    if (retry && !pending) {
      card.append(element("p", "retry-note", "The earlier result wasn't confirmed. Retry safely without reserving twice."));
    }
    ui.concerts.append(card);
  });
}

function renderHistory() {
  ui["reservations-section"].hidden = !principal;
  ui["reservation-history"].setAttribute("aria-busy", String(historyLoading));
  ui["refresh-reservations"].disabled = historyLoading || !principal;
  ui["refresh-reservations"].textContent = historyLoading ? "Refreshing…" : "Refresh reservations";
  ui["reservation-summary"].textContent = historyLoading ? "Checking your reservations." :
    historyFailed ? "Your reservations couldn't be refreshed. Please try again." :
    activeLimit === null ? `${activeCount} active ${activeCount === 1 ? "seat" : "seats"}. No personal limit.` :
      `${activeCount} of ${activeLimit} active seats.`;
  ui["allowance-message"].hidden = !principal || !atLimit();
  ui["allowance-message"].textContent = "Your current allowance is full. Cancel a reservation to make room for another concert.";
  ui["reservation-history"].replaceChildren();
  reservationHeadings.clear();
  if (!principal) return;
  if (!reservations.length) {
    ui["reservation-history"].append(element("p", "empty-state", historyLoading ? "Loading your reservations…" :
      historyFailed ? "Your history couldn't be loaded. Use Refresh reservations to try again." :
      "No reservations yet. Choose a concert above to reserve your first seat."));
    return;
  }
  const list = element("ul", "reservation-list");
  for (const reservation of reservations) {
    const concert = concerts.find(item => item.id === reservation.concert_id);
    const title = concert ? concert.title : reservation.concert_id;
    const row = element("li", "reservation-item");
    row.dataset.reservationId = reservation.id;
    const detail = element("div", "reservation-detail");
    const heading = element("h3", "", title);
    heading.tabIndex = -1;
    reservationHeadings.set(reservation.id, heading);
    detail.append(heading, element("p", "reservation-reference", `Reference ${reservation.id}`));
    const actions = element("div", "reservation-actions");
    actions.append(element("span", `reservation-state state-${reservation.state}`,
      reservation.state === "reserved" ? "Reserved" : "Cancelled"));
    if (reservation.state === "reserved") {
      const button = element("button", "button-secondary", cancelBusy.has(reservation.id) ? "Cancelling…" : "Cancel reservation");
      button.type = "button";
      button.disabled = cancelBusy.has(reservation.id);
      button.setAttribute("aria-label", `Cancel reservation for ${title}`);
      button.addEventListener("click", () => cancel(reservation.id));
      actions.append(button);
    }
    row.append(detail, actions);
    list.append(row);
  }
  ui["reservation-history"].append(list);
}

function render() {
  renderSession();
  renderConcerts();
  renderHistory();
}

function validConcerts(data) {
  if (!Array.isArray(data.concerts) || data.concerts.some(concert =>
    ["id", "title", "date", "venue"].some(field => typeof concert[field] !== "string") ||
    !Number.isInteger(concert.capacity) || !Number.isInteger(concert.available) ||
    concert.capacity < 0 || concert.available < 0 || concert.available > concert.capacity)) {
    throw new Error("Unconfirmed catalog response.");
  }
  return data.concerts;
}

async function refreshCatalog() {
  const epoch = identityEpoch;
  const sequence = ++catalogRequest;
  catalogLoading = true;
  renderConcerts();
  try {
    const data = validConcerts(await request("/api/concerts"));
    if (epoch !== identityEpoch || sequence !== catalogRequest) return;
    concerts = data;
    catalogFailed = false;
  } catch (failure) {
    if (epoch !== identityEpoch || sequence !== catalogRequest) return;
    catalogFailed = true;
    error("The program couldn't be refreshed. Check your connection and try again.");
  } finally {
    if (epoch === identityEpoch && sequence === catalogRequest) {
      catalogLoading = false;
      renderConcerts();
      renderHistory();
    }
  }
}

async function refreshHistory() {
  if (!principal) return;
  const epoch = identityEpoch;
  const sequence = ++historyRequest;
  historyLoading = true;
  renderHistory();
  try {
    const data = await request("/api/reservations");
    if (epoch !== identityEpoch || sequence !== historyRequest) return;
    if (!Array.isArray(data.reservations) || !Number.isInteger(data.active_count) || data.active_count < 0 ||
      !(data.active_limit === null || (Number.isInteger(data.active_limit) && data.active_limit > 0)) ||
      data.reservations.some(item => typeof item.id !== "string" || typeof item.concert_id !== "string" ||
        !["reserved", "cancelled"].includes(item.state))) throw new Error("Unconfirmed history response.");
    reservations = data.reservations;
    activeCount = data.active_count;
    activeLimit = data.active_limit;
    historyFailed = false;
  } catch (failure) {
    if (epoch !== identityEpoch || sequence !== historyRequest) return;
    if (["Unauthenticated", "SessionChanged"].includes(failure.code)) endExpiredSession(epoch);
    else {
      historyFailed = true;
      error("Your reservations couldn't be refreshed. Check your connection and try again.");
    }
  } finally {
    if (epoch === identityEpoch && sequence === historyRequest) {
      historyLoading = false;
      renderHistory();
      renderConcerts();
    }
  }
}

async function refresh() {
  await Promise.all([refreshCatalog(), refreshHistory()]);
}

function requestKey() {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, "0")).join("");
}

async function reserve(concertId) {
  if (!principal || reserveBusy.has(concertId)) return;
  const epoch = identityEpoch;
  const title = concerts.find(item => item.id === concertId)?.title || "This concert";
  if (!reserveKeys.has(concertId)) reserveKeys.set(concertId, requestKey());
  const key = reserveKeys.get(concertId);
  reserveBusy.add(concertId);
  error("");
  status(`Reserving a seat for ${title}…`);
  renderConcerts();
  try {
    const data = await request("/api/reservations", { concert_id: concertId, request_key: key });
    if (epoch !== identityEpoch) return;
    if (typeof data.id !== "string" || !["reserved", "cancelled"].includes(data.state)) throw new Error("Unconfirmed reservation response.");
    reserveKeys.delete(concertId);
    status(data.state === "reserved" ? `Your seat for ${title} is reserved.` :
      "Your earlier reservation is cancelled. This retry did not reserve another seat.");
  } catch (failure) {
    if (epoch !== identityEpoch) return;
    if (failure instanceof ApiError && ["Unauthenticated", "SessionChanged", "InvalidInput", "ConcertNotFound",
      "SoldOut", "IdempotencyConflict", "ActiveReservationLimitReached", "PersistenceUnavailable"].includes(failure.code)) {
      reserveKeys.delete(concertId);
      if (["Unauthenticated", "SessionChanged"].includes(failure.code)) endExpiredSession(epoch);
      else {
        error(errorText(failure));
        status("No new reservation was made.");
      }
    } else {
      error("We couldn't confirm your reservation. Use Retry reservation to check the same request safely without reserving twice.");
      status("Your reservation request is ready for a safe retry.");
    }
  } finally {
    if (epoch === identityEpoch) {
      reserveBusy.delete(concertId);
      renderConcerts();
      await refresh();
      if (epoch === identityEpoch) {
        const button = concertButtons.get(concertId);
        if (button && !button.disabled) button.focus();
        else concertHeadings.get(concertId)?.focus();
      }
    }
  }
}

async function cancel(reservationId) {
  if (!principal || cancelBusy.has(reservationId)) return;
  const epoch = identityEpoch;
  cancelBusy.add(reservationId);
  error("");
  status("Cancelling your reservation…");
  renderHistory();
  try {
    const data = await request(`/api/reservations/${encodeURIComponent(reservationId)}/cancel`, {});
    if (epoch !== identityEpoch) return;
    if (data.id !== reservationId || data.state !== "cancelled") throw new Error("Unconfirmed cancellation response.");
    status("Your reservation is cancelled. It no longer occupies a seat.");
  } catch (failure) {
    if (epoch !== identityEpoch) return;
    if (["Unauthenticated", "SessionChanged"].includes(failure.code)) endExpiredSession(epoch);
    else {
      error(failure instanceof ApiError ? errorText(failure) :
        "We couldn't confirm the cancellation. Refresh your history; you can safely repeat a cancellation.");
      status("Check your reservation history before retrying.");
    }
  } finally {
    if (epoch === identityEpoch) {
      cancelBusy.delete(reservationId);
      await refresh();
      if (epoch === identityEpoch) reservationHeadings.get(reservationId)?.focus();
    }
  }
}

ui["sign-in-form"].addEventListener("submit", async event => {
  event.preventDefault();
  if (initializing || signingIn || signingOut || signOutUncertain) return;
  const credentials = { username: ui.username.value, password: ui.password.value };
  signingIn = true;
  let epoch = setIdentity(null);
  error("");
  status("Signing in…");
  try {
    const data = await request("/api/login", credentials);
    if (epoch !== identityEpoch) return;
    if (!data.principal || typeof data.principal.id !== "string" || typeof data.principal.name !== "string") throw new Error("Unconfirmed sign-in response.");
    epoch = setIdentity(data.principal);
    status(`Welcome, ${principal.name}. Choose a concert to reserve your seat.`);
    await refresh();
    if (epoch === identityEpoch) ui["program-heading"].focus();
  } catch (failure) {
    if (epoch !== identityEpoch) return;
    error(failure instanceof ApiError ? errorText(failure) : "Sign-in couldn't be confirmed. Please try signing in again.");
    status("Sign in to reserve a seat.");
  } finally {
    signingIn = false;
    if (epoch === identityEpoch) {
      render();
      if (!principal) ui.username.focus();
    } else renderSession();
  }
});

ui["sign-out"].addEventListener("click", async () => {
  if (signingOut || signingIn) return;
  if (!signOutUncertain) signOutExpected = principal?.id;
  const expectedPrincipal = signOutExpected;
  signingOut = true;
  signOutUncertain = false;
  const epoch = setIdentity(null);
  error("");
  status("Signing out…");
  try {
    const data = await request("/api/logout", {}, expectedPrincipal);
    if (epoch !== identityEpoch) return;
    if (data.principal !== null) throw new Error("Unconfirmed sign-out response.");
    signOutExpected = null;
    status("You're signed out. Your reservation history stays with your account.");
  } catch (failure) {
    if (epoch !== identityEpoch) return;
    if (failure.code === "SessionChanged") {
      signOutExpected = null;
      status("The session changed in another tab. Sign in again.");
      error("The other session was preserved. Sign in to choose your account.");
    } else {
      signOutUncertain = true;
      error("Sign-out couldn't be confirmed. Use Retry sign out before signing in again.");
    }
  } finally {
    if (epoch === identityEpoch) {
      signingOut = false;
      render();
      if (!signOutUncertain) ui.username.focus();
      await refreshCatalog();
    }
  }
});

ui["refresh-concerts"].addEventListener("click", async () => {
  error("");
  await refreshCatalog();
});
ui["refresh-reservations"].addEventListener("click", async () => {
  error("");
  await refresh();
});

async function start() {
  render();
  const epoch = identityEpoch;
  const [session, catalog] = await Promise.allSettled([request("/api/session"), request("/api/concerts")]);
  if (epoch !== identityEpoch) return;
  initializing = false;
  catalogLoading = false;
  const profile = session.status === "fulfilled" ? session.value.principal : null;
  setIdentity(profile && typeof profile.id === "string" && typeof profile.name === "string" ? profile : null);
  if (catalog.status === "fulfilled") {
    try { concerts = validConcerts(catalog.value); }
    catch (failure) { catalogFailed = true; }
  } else catalogFailed = true;
  if (catalogFailed) error("The program couldn't be loaded. Use Refresh concerts to try again.");
  else if (session.status === "rejected") error("Your session couldn't be checked. Sign in again before reserving a seat.");
  status(principal ? `Welcome back, ${principal.name}.` : "Sign in to reserve a seat, or browse the program below.");
  render();
  if (principal) await refreshHistory();
}

start();
