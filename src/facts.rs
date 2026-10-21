//! Coffee facts: a little personality in the menu.
//!
//! One fact a day sits at the top of the menu; clicking it shows the next one.
//! At most once a day, starting a session also brings one up as a
//! notification. Never more often than that: a utility that keeps
//! interrupting with trivia stops being charming by the third time.
//!
//! Every line here should survive a fact-check. Popular coffee "facts" are
//! often folklore; where the story is a legend, it says so.

pub const FACTS: &[&str] = &[
    "Brazil is the world's largest coffee producer and exporter.",
    "Brazil grows roughly a third of all the world's coffee.",
    "Vietnam is the world's second-largest coffee producer.",
    "Colombia grows almost exclusively arabica coffee.",
    "A traditional cappuccino is \u{2153} espresso, \u{2153} milk, \u{2153} foam.",
    "Coffee beans are the seeds of a fruit: the coffee cherry.",
    "A cherry usually holds two seeds. A single round one is a peaberry.",
    "Arabica and robusta are the two species behind almost all coffee.",
    "Robusta has roughly twice the caffeine of arabica.",
    "Coffee plants are native to Ethiopia.",
    "Legend says an Ethiopian goatherd, Kaldi, discovered coffee.",
    "Coffee was first brewed as a drink in Yemen, in the 15th century.",
    "Mocha is named after the port of Mocha, in Yemen.",
    "Coffee houses were flourishing in Constantinople by the 1550s.",
    "Coffee reached Europe through Venice in the 17th century.",
    "Coffee grows in the 'Bean Belt', between the two Tropics.",
    "A coffee tree takes three to four years to bear its first fruit.",
    "Caffeine is the coffee plant's natural defence against insects.",
    "'Latte' is simply the Italian word for milk.",
    "'Macchiato' means 'stained' or 'marked' in Italian.",
    "A cortado is espresso 'cut' with a similar amount of warm milk.",
    "Affogato means 'drowned': gelato drowned in a shot of espresso.",
    "A ristretto is a 'restricted' espresso: less water, more intensity.",
    "A lungo is a 'long' espresso, pulled with more water.",
    "An americano is espresso topped up with hot water.",
    "The flat white was popularised in Australia and New Zealand.",
    "Crema is the golden foam of oils and CO\u{2082} on top of an espresso.",
    "A shot of espresso has less caffeine than a mug of drip coffee.",
    "Decaf isn't caffeine-free: a small amount always remains.",
    "Cold brew steeps in cold water for 12 to 24 hours.",
    "Green coffee is roasted at around 200 \u{b0}C to develop its flavour.",
    "Angelo Moriondo patented an espresso machine in Turin in 1884.",
    "Melitta Bentz invented the paper coffee filter in 1908.",
    "Turkish coffee is served unfiltered, grounds settled in the cup.",
    "Turkish coffee culture is on UNESCO's intangible heritage list.",
    "An Ethiopian coffee ceremony serves three rounds of cups.",
    "In Brazil, a cafezinho is the small sweet coffee offered as welcome.",
    "Finland usually ranks first in coffee drunk per person.",
    "Kopi luwak is made from beans that passed through a civet.",
    "Coffee is one of the most traded agricultural commodities.",
];

/// Days since the Unix epoch: the index that turns the fact over once a day.
pub fn today() -> u64 {
    crate::util::now_unix() / 86_400
}

/// The fact for a given day, `offset` places further on (one per click).
pub fn for_day(day: u64, offset: usize) -> &'static str {
    FACTS[(day as usize).wrapping_add(offset) % FACTS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_fit_a_menu_row_and_are_unique() {
        for f in FACTS {
            // A menu row does not wrap; a long one widens the whole menu.
            assert!(f.chars().count() <= 70, "too long for the menu: {f}");
        }
        let mut seen = std::collections::HashSet::new();
        assert!(FACTS.iter().all(|f| seen.insert(f)), "duplicate fact");
    }

    #[test]
    fn rotates_daily_and_wraps() {
        assert_ne!(for_day(0, 0), for_day(1, 0));
        assert_eq!(for_day(0, 1), for_day(1, 0));
        assert_eq!(for_day(0, FACTS.len()), for_day(0, 0));
    }
}
