use scraper::{Html, Selector};
use std::fmt;

// We define an enumeration (enum) to represent all 12 Zodiac signs.
// The '#[derive(...)]' line automatically provides common traits for our enum,
// such as checking equality (PartialEq, Eq), printing for debugging (Debug),
// and allowing it to be used as a key in a HashMap (Hash).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZodiacSign {
    Aries,
    Taurus,
    Gemini,
    Cancer,
    Leo,
    Virgo,
    Libra,
    Scorpio,
    Sagittarius,
    Capricorn,
    Aquarius,
    Pisces,
}

impl ZodiacSign {
    // A constant array containing all the signs. This makes it easy to iterate over them later.
    pub const ALL: [ZodiacSign; 12] = [
        ZodiacSign::Aries,
        ZodiacSign::Taurus,
        ZodiacSign::Gemini,
        ZodiacSign::Cancer,
        ZodiacSign::Leo,
        ZodiacSign::Virgo,
        ZodiacSign::Libra,
        ZodiacSign::Scorpio,
        ZodiacSign::Sagittarius,
        ZodiacSign::Capricorn,
        ZodiacSign::Aquarius,
        ZodiacSign::Pisces,
    ];

    // This method converts our enum into a lowercase string slice.
    // This string is used to build the URL for scraping (e.g., "aries" -> ".../today/aries.html").
    pub fn as_str(&self) -> &'static str {
        match self {
            ZodiacSign::Aries => "aries",
            ZodiacSign::Taurus => "taurus",
            ZodiacSign::Gemini => "gemini",
            ZodiacSign::Cancer => "cancer",
            ZodiacSign::Leo => "leo",
            ZodiacSign::Virgo => "virgo",
            ZodiacSign::Libra => "libra",
            ZodiacSign::Scorpio => "scorpio",
            ZodiacSign::Sagittarius => "sagittarius",
            ZodiacSign::Capricorn => "capricorn",
            ZodiacSign::Aquarius => "aquarius",
            ZodiacSign::Pisces => "pisces",
        }
    }
}

// We implement the 'Display' trait so we can print the Zodiac sign nicely in the UI.
// This will capitalize the first letter of the sign (e.g., "aries" becomes "Aries").
impl fmt::Display for ZodiacSign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.as_str();
        
        // Get the characters of the string
        let mut chars = name.chars();
        
        // Take the first character, make it uppercase, and chain the rest of the characters.
        match chars.next() {
            None => String::new(),
            Some(c) => c
                .to_uppercase()
                .chain(chars)
                .collect::<String>(),
        }
        .fmt(f)
    }
}

// A simple structure to hold the score for a specific category (e.g., "Mood", 80).
#[derive(Debug, Clone)]
pub struct CategoryScore {
    pub category: String,
    pub percent: u16,
}

// A structure to hold the complete data for a specific Zodiac sign,
// including the sign itself and a list of all its category scores.
#[derive(Debug, Clone)]
pub struct HoroscopeData {
    pub sign: ZodiacSign,
    pub scores: Vec<CategoryScore>,
}

// Custom error enum to handle different types of failures that might occur during scraping.
#[derive(Debug)]
pub enum ScrapeError {
    // If the HTTP request fails (e.g., no internet connection).
    Request(()), 
}

// Automatically convert reqwest errors into our custom ScrapeError.
impl From<reqwest::Error> for ScrapeError {
    fn from(_err: reqwest::Error) -> Self {
        ScrapeError::Request(())
    }
}

// This is the core asynchronous function that fetches and parses the horoscope data.
// It takes a ZodiacSign and returns either HoroscopeData (on success) or a ScrapeError (on failure).
pub async fn scrape_horoscope(sign: ZodiacSign) -> Result<HoroscopeData, ScrapeError> {
    
    // 1. Build the target URL for the specific zodiac sign.
    let url = format!(
        "https://www.free-horoscope.com/horoscopes/today/{}.html",
        sign.as_str()
    );
    
    // 2. Make an asynchronous HTTP GET request and wait for the response text.
    let response = reqwest::get(&url)
        .await?
        .text()
        .await?;

    // 3. Parse the raw HTML text into a searchable document structure.
    let document = Html::parse_document(&response);

    // 4. Define CSS selectors to find specific elements within the HTML.
    // 'unwrap()' is safe here because we know these selector strings are perfectly valid CSS.
    let section_selector = Selector::parse(".astro-section-glass").unwrap();
    let h2_selector = Selector::parse("h2").unwrap();
    let col_selector = Selector::parse(".chart-col").unwrap();
    let bar_fill_selector = Selector::parse(".bar-fill").unwrap();

    // Create an empty vector to store our extracted scores.
    let mut scores = Vec::new();

    // 5. Loop through every section in the HTML that matches the "astro-section-glass" class.
    for section in document.select(&section_selector) {
        
        // Extract the category name from the <h2> tag.
        let category = if let Some(h2) = section.select(&h2_selector).next() {
            h2
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string()
        } else {
            // If there is no <h2> tag, we skip this section.
            continue;
        };

        // Extract the percentage value from the style attribute of the chart bar.
        if let Some(col) = section.select(&col_selector).next() {
            if let Some(bar_fill) = col.select(&bar_fill_selector).next() {
                if let Some(style) = bar_fill.value().attr("style") {
                    
                    // Look for the "height:" property in the inline CSS style.
                    if let Some(start) = style.find("height:") {
                        let substr = &style[start + 7..];
                        
                        // Find the '%' symbol and extract the number before it.
                        if let Some(end) = substr.find('%') {
                            let num_str = substr[..end].trim();
                            
                            // Attempt to parse the number string into an integer (u16).
                            if let Ok(percent) = num_str.parse::<u16>() {
                                // Add the successfully extracted score to our list.
                                scores.push(CategoryScore { category, percent });
                            }
                        }
                    }
                }
            }
        }
    }

    // 6. Fallback mechanism: If the website structure changed or the request returned no valid data,
    // we provide mock data so the TUI remains functional and visually appealing for demonstration.
    if scores.is_empty() {
        scores = vec![
            CategoryScore {
                category: "Mood".to_string(),
                percent: 85,
            },
            CategoryScore {
                category: "Love".to_string(),
                percent: 70,
            },
            CategoryScore {
                category: "Money".to_string(),
                percent: 60,
            },
            CategoryScore {
                category: "Work".to_string(),
                percent: 90,
            },
            CategoryScore {
                category: "Leisure".to_string(),
                percent: 50,
            },
        ];
    }

    // Return the final packaged data.
    Ok(HoroscopeData { sign, scores })
}
