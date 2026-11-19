//! The small subset of the WebDriver HTTP protocol used by the concert regression.

use std::{
    env,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use ureq::Agent;

const ELEMENT: &str = "element-6066-11e4-a52e-4f735466cecf";
const TIMEOUT: Duration = Duration::from_secs(25);

pub struct Browser {
    driver: Child,
    client: Agent,
    session: String,
}

impl Browser {
    pub fn start() -> Self {
        Self::with_preferences(json!({}))
    }

    pub fn with_preferences(mut preferences: Value) -> Self {
        preferences["profile.managed_default_content_settings.javascript"] = json!(2);
        let executable = env::var_os("GDD_CHROMEDRIVER").unwrap_or_else(|| "chromedriver".into());
        let driver = Command::new(&executable)
            .arg("--port=0")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("Cannot start {executable:?}: {e}. Set GDD_CHROMEDRIVER to a compatible local ChromeDriver."));
        let mut browser = Self {
            driver,
            client: Agent::config_builder()
                .proxy(None)
                .timeout_global(Some(TIMEOUT))
                .http_status_as_error(false)
                .build()
                .into(),
            session: String::new(),
        };
        let stdout = browser.driver.stdout.take().unwrap();
        let (ready, address) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(port) =
                    line.strip_prefix("ChromeDriver was started successfully on port ")
                {
                    let _ = ready.send(port.trim_end_matches('.').parse::<u16>());
                    break;
                }
            }
        });
        let port = address
            .recv_timeout(TIMEOUT)
            .expect("ChromeDriver startup")
            .expect("ChromeDriver port");
        let url = format!("http://127.0.0.1:{port}/session");
        // ChromeDriver creates a fresh temporary profile and removes it on Delete Session.
        let capabilities = json!({"capabilities": {"alwaysMatch": {
            "browserName": "chrome",
            "timeouts": {"pageLoad": 15000, "implicit": 0},
            "goog:chromeOptions": {
                "args": ["--headless", "--disable-dev-shm-usage", "--no-first-run"],
                "prefs": preferences
            }
        }}});
        let session = browser.command(&url, Some(capabilities));
        browser.session = format!(
            "{url}/{}",
            session["sessionId"].as_str().expect("WebDriver session ID")
        );
        browser
    }

    #[track_caller]
    fn command(&self, url: &str, body: Option<Value>) -> Value {
        let result = match body {
            Some(data) => self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .send(data.to_string()),
            None => self.client.get(url).call(),
        };
        let mut response = result.unwrap_or_else(|e| panic!("WebDriver {url}: {e}"));
        let status = response.status();
        let mut json: Value =
            serde_json::from_reader(response.body_mut().as_reader()).expect("WebDriver JSON");
        assert!(
            status.is_success(),
            "WebDriver {url} returned {status}: {json}"
        );
        json.get_mut("value").expect("WebDriver value").take()
    }

    #[track_caller]
    pub fn get(&self, path: &str) -> Value {
        self.command(&format!("{}{path}", self.session), None)
    }

    #[track_caller]
    pub fn post(&self, path: &str, body: Value) -> Value {
        self.command(&format!("{}{path}", self.session), Some(body))
    }

    fn element(&self, css: &str) -> String {
        let element = self.post("/element", json!({"using":"css selector", "value":css}));
        element[ELEMENT].as_str().expect("element ID").to_owned()
    }

    fn elements(&self, css: &str) -> Value {
        self.post("/elements", json!({"using":"css selector", "value":css}))
    }

    pub fn count(&self, css: &str) -> usize {
        self.elements(css)
            .as_array()
            .expect("WebDriver elements")
            .len()
    }

    pub fn text(&self, css: &str) -> String {
        self.get(&format!("/element/{}/text", self.element(css)))
            .as_str()
            .unwrap()
            .to_owned()
    }

    pub fn value(&self, css: &str) -> String {
        self.get(&format!("/element/{}/attribute/value", self.element(css)))
            .as_str()
            .unwrap()
            .to_owned()
    }

    pub fn rect(&self, css: &str) -> Value {
        self.get(&format!("/element/{}/rect", self.element(css)))
    }

    pub fn visible_rects(&self, css: &str) -> Vec<Value> {
        let elements = self.elements(css);
        let mut rects = Vec::new();
        for element in elements.as_array().expect("WebDriver elements") {
            let id = element[ELEMENT].as_str().expect("element ID");
            if self.get(&format!("/element/{id}/displayed")) == true {
                rects.push(self.get(&format!("/element/{id}/rect")));
            }
        }
        rects
    }

    pub fn focused(&self, css: &str) -> bool {
        self.get("/element/active")[ELEMENT] == self.element(css)
    }

    pub fn keys(&self, text: &str) {
        let mut actions = Vec::new();
        for key in text.chars() {
            actions.push(json!({"type": "keyDown", "value": key}));
            actions.push(json!({"type": "keyUp", "value": key}));
        }
        self.post(
            "/actions",
            json!({"actions": [{"type": "key", "id": "keyboard", "actions": actions}]}),
        );
    }

    pub fn cdp(&self, command: &str, params: Value) -> Value {
        self.post(
            "/goog/cdp/execute",
            json!({"cmd": command, "params": params}),
        )
    }

    pub fn click(&self, css: &str) {
        self.navigate_with(|| {
            self.post(&format!("/element/{}/click", self.element(css)), json!({}));
        });
    }

    // A form click or Enter can return before the new desk or error page replaces the form.
    pub fn navigate_with(&self, action: impl FnOnce()) {
        let previous = self.element("html");
        action();
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let document = self.elements("html");
            if document[0][ELEMENT]
                .as_str()
                .is_some_and(|id| id != previous)
            {
                return;
            }
            assert!(Instant::now() < deadline, "form did not navigate");
            thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn type_text(&self, css: &str, text: &str) {
        self.post(
            &format!("/element/{}/value", self.element(css)),
            json!({"text":text}),
        );
    }

    pub fn open(&self, url: &str) {
        self.post("/url", json!({"url":url}));
    }

    pub fn switch(&self, handle: &Value) {
        self.post("/window", json!({"handle":handle}));
    }

    pub fn new_tab(&self, url: &str) -> Value {
        let handle = self.post("/window/new", json!({"type":"tab"}))["handle"].clone();
        self.switch(&handle);
        self.open(url);
        handle
    }

    pub fn session_cookie(&self) -> String {
        let cookie = self.get("/cookie/concert_session");
        assert_eq!(cookie["httpOnly"], true);
        assert_eq!(cookie["sameSite"], "Strict");
        format!("concert_session={}", cookie["value"].as_str().unwrap())
    }

    pub fn api_history(&self, site: &str) -> Value {
        self.api_history_with_cookie(site, &self.session_cookie())
    }

    pub fn api_history_with_cookie(&self, site: &str, cookie: &str) -> Value {
        let mut response = self
            .client
            .get(format!("{site}/api/reservations"))
            .header("Cookie", cookie)
            .call()
            .expect("reservation API response");
        assert_eq!(response.status(), 200);
        serde_json::from_reader(response.body_mut().as_reader()).expect("reservation history")
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let quit = (!self.session.is_empty()).then(|| {
            self.client
                .delete(&self.session)
                .config()
                .http_status_as_error(true)
                .build()
                .call()
        });
        let _ = self.driver.kill();
        let _ = self.driver.wait();
        if let Some(Err(error)) = quit {
            if thread::panicking() {
                eprintln!("ChromeDriver session cleanup failed: {error}");
            } else {
                panic!("ChromeDriver session cleanup failed: {error}");
            }
        }
    }
}
