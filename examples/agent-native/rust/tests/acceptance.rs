// The two leaf actions share the same acceptance cases.
mod discount {
    use quote_demo::discount as calculate;
    include!("scenarios/leaf.rs");
}

mod tax {
    use quote_demo::tax as calculate;
    include!("scenarios/leaf.rs");
}

#[path = "scenarios/quote.rs"]
mod quote;
