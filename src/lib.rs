use std::{fmt, io};
use std::fmt::Formatter;
use reqwest::Response;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use log::{debug, error};
use thiserror::Error;

// md parsing constants
const METADATA_DELIM: &str = "---";
const METADATA_KEY_VALUE_DELIM: &str = ":";
const DECK: &str = "deck";

// api constants
const API_VERSION: i32 = 6;

/// Anki note models
#[derive(Clone, Debug)]
enum Model {
    Basic,
}

impl Model {
    fn to_str(&self) -> &str {
        match self {
            Model::Basic => "Basic",
        }
    }
}

/// A deck in anki
#[derive(Clone, Debug, Deserialize)]
struct Deck(String);

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Received reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Received API error: {0}")]
    ResponseError(String),
}

/// addNotes response type
#[derive(Debug, Deserialize)]
struct AddNotesResponse {
    result: Vec<Option<i64>>,
    error: Option<String>,
}

/// Small client for using anki-connect's APIs
struct AnkiConnectClient {
    endpoint: String,
    client: reqwest::Client,
}

impl AnkiConnectClient {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_owned(),
            client: reqwest::Client::new(),
        }
    }

    pub fn default() -> Self {
        Self::new("http://localhost:8765")
    }

    pub async fn post(&self, body: &Value) -> Result<Response, reqwest::Error> {
        debug!("Sending post with body: {:?}", body);
        let response = self.client
            .post(&self.endpoint)
            .json(body)
            .send()
            .await;
        debug!("Received response: {:?}", response);
        response
    }

    pub async fn add_notes(&self, notes: Vec<ParsedNote>) -> Result<(), ApiError> {
        let notes_json: Vec<Value> = notes.iter().map(|n| n.to_json()).collect();
        let body = json!({
            "action": "addNotes",
            "version": API_VERSION,
            "params": {
                "notes": notes_json
            }
        });

        let response = self.post(&body).await?;
        let add_notes_response = response.json::<AddNotesResponse>().await?;
        
        match add_notes_response.error {
            Some(e) => Err(ApiError::ResponseError(e)),
            None => {
                debug!("Response: {:?}", add_notes_response.result);
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedNote {
    deck: Deck,
    model: Model,
    question: String,
    answer: String,
}

impl ParsedNote {
    fn to_json(&self) -> Value {
        json!({
            "deckName": self.deck.0,
            "modelName": self.model.to_str(),
            "fields": {
                "Front": self.question,
                "Back": self.answer
            },
            "tags": [],
            "options": {
                "allowDuplicate": false,
                "duplicateScope": "deck"
            }
        })
    }
}

#[derive(Error, Debug)]
pub enum ParseError {
    // TODO: improve messages
    #[error("Invalid state change on line {0}")]
    InvalidStateChange(u128),
    #[error("Unexpected parsing end")] // TODO: make this better
    UnexpectedParsingEnd,
    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),
    #[error("Unable to determine which deck card belongs to")]
    MissingDeck,
    #[error("IO error on file {0}")]
    IOError(#[from] io::Error),
}

#[derive(Clone, Debug)]
enum ParseState {
    Start,
    InMetadata,
    ExpectingQuestion,
    InQuestion,
    InAnswer,
}

impl fmt::Display for ParseState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ParseState::Start => write!(f, "Start"),
            ParseState::InMetadata => write!(f, "InMetadata"),
            ParseState::ExpectingQuestion => write!(f, "ExpectingQuestion"),
            ParseState::InQuestion => write!(f, "InQuestion"),
            ParseState::InAnswer => write!(f, "InAnswer"),
        }
    }
}

impl ParseState {
    pub fn next(&self) -> Self {
        match self {
            ParseState::Start => ParseState::InMetadata,
            ParseState::InMetadata => ParseState::ExpectingQuestion,
            ParseState::ExpectingQuestion => ParseState::InQuestion,
            ParseState::InQuestion => ParseState::InAnswer,
            ParseState::InAnswer => ParseState::InQuestion,
        }
    }

    pub fn reset(&self) -> Result<Self, ParseError> {
        match self {
            ParseState::InAnswer => Ok(ParseState::Start),
            _ => Err(ParseError::UnexpectedParsingEnd),
        }
    }
}

enum ParseEventType<'a> {
    MetadataDelimiter, // metadata delimiter, i.e. '---'
    QuestionStart(&'a str), // start of a question, default is 'Q: '
    AnswerStart(&'a str), // start of an answer, default is 'A: '
    Text(&'a str), // text, could be metadata, answer, or question
    Empty, // an empty string
}

struct Parser {
    state: ParseState,         // internal parser state
    question_token: String,    // token used to delimit question strings
    question_token_len: usize, // size of the question token used
    answer_token: String,      // token used to delimit answer strings
    answer_token_len: usize,   // size of the answer token used
    deck: Option<Deck>, // current deck we are modifying, from the metadata at the top of the file
    question: String, // internal state of the current question being parsed
    answer: String,   // internal state of the current answer being parsed
    parsed: Vec<ParsedNote>, // the results that will be retrieved in `finalize()`
    line_num: u128, // counter to let us keep track of the line number in a file
}

impl Parser {
    fn new(question_token: &str, answer_token: &str) -> Self {
        Self {
            state: ParseState::Start,
            question_token: question_token.to_owned(),
            question_token_len: question_token.chars().count(),
            answer_token: answer_token.to_owned(),
            answer_token_len: answer_token.chars().count(),
            deck: None,
            question: String::new(),
            answer: String::new(),
            parsed: Vec::new(),
            line_num: 0,
        }
    }

    fn handle_event(&mut self, event: &str) -> Result<(), ParseError> {
        // pass things from self to parse_event_type to avoid having a mutable and immutable borrow
        // for self in this function. probably a better way to fix it but oh well
        let event_type = Parser::parse_event_type(
            event,
            &self.question_token,
            self.question_token_len,
            &self.answer_token,
            self.answer_token_len,
        );
        debug!("Parsing line: {}", event);
        let result = self.parse(event_type);
        self.line_num += 1;
        result
    }

    fn parse_event_type<'a>(
        event: &'a str,
        question_token: &str,
        question_token_len: usize,
        answer_token: &str,
        answer_token_len: usize,
    ) -> ParseEventType<'a> {
        return if event.starts_with(METADATA_DELIM) {
            ParseEventType::MetadataDelimiter
        } else if event.starts_with(&question_token) {
            ParseEventType::QuestionStart(&event[question_token_len..]) // remove token
        } else if event.starts_with(&answer_token) {
            ParseEventType::AnswerStart(&event[answer_token_len..]) // remove token
        } else if event.is_empty() {
            ParseEventType::Empty
        } else {
            ParseEventType::Text(event)
        };
    }

    fn parse(&mut self, event_type: ParseEventType) -> Result<(), ParseError> {
        match event_type {
            ParseEventType::MetadataDelimiter => self.handle_metadata_delim(),
            ParseEventType::QuestionStart(event) => self.handle_question_start(event),
            ParseEventType::AnswerStart(event) => self.handle_answer_start(event),
            ParseEventType::Text(event) => self.handle_text(event),
            ParseEventType::Empty => self.handle_empty(),
        }
    }

    fn finalize_note(&mut self) -> Result<(), ParseError> {
        let deck = self.deck.as_ref().ok_or(ParseError::MissingDeck)?;

        self.parsed.push(ParsedNote {
            model: Model::Basic,
            deck: deck.clone(),
            question: markdown::to_html(&self.question),
            answer: markdown::to_html(&self.answer),
        });

        self.question.clear();
        self.answer.clear();

        Ok(())
    }

    fn handle_metadata_delim(&mut self) -> Result<(), ParseError> {
        match &self.state {
            ParseState::Start | ParseState::InMetadata => {
                self.state = self.state.next();
                Ok(())
            }
            _ => Err(ParseError::InvalidStateChange(self.line_num)),
        }
    }

    fn handle_question_start(&mut self, event: &str) -> Result<(), ParseError> {
        match &mut self.state {
            ParseState::ExpectingQuestion => {
                self.question.push_str(&format!("{}\n", event));
                self.state = self.state.next();
                Ok(())
            }
            ParseState::InAnswer => {
                self.finalize_note()?;
                self.question.push_str(&format!("{}\n", event));
                self.state = self.state.next();
                Ok(())
            }
            _ => Err(ParseError::InvalidStateChange(self.line_num)),
        }
    }

    fn handle_answer_start(&mut self, event: &str) -> Result<(), ParseError> {
        match &mut self.state {
            ParseState::InQuestion => {
                self.answer.push_str(&format!("{}\n", event));
                self.state = self.state.next();
                Ok(())
            }
            _ => Err(ParseError::InvalidStateChange(self.line_num)),
        }
    }

    fn handle_text(&mut self, event: &str) -> Result<(), ParseError> {
        match &self.state {
            ParseState::InQuestion => {
                self.question.push_str(&format!("{}\n", event));
                Ok(())
            }
            ParseState::InAnswer => {
                self.answer.push_str(&format!("{}\n", event));
                Ok(())
            }
            ParseState::InMetadata => {
                let split: Vec<&str> = event.split(METADATA_KEY_VALUE_DELIM).collect();
                if split.len() != 2 {
                    return Err(ParseError::InvalidMetadata(format!(
                        "Unable to parse line: {}",
                        event
                    )));
                } else if split[0] != DECK {
                    return Err(ParseError::InvalidMetadata(format!(
                        "Expecting 'deck' keyword, found: {}",
                        event
                    )));
                } else {
                    let deck = Deck(split[1].trim().to_owned());
                    self.deck = Some(deck);
                }

                Ok(())
            }
            _ => Err(ParseError::InvalidStateChange(self.line_num)),
        }
    }

    fn handle_empty(&mut self) -> Result<(), ParseError> {
        match self.state {
            ParseState::InQuestion => self.question.push('\n'),
            ParseState::InAnswer => self.answer.push('\n'),
            _ => (),
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<Vec<ParsedNote>, ParseError> {
        match &self.state {
            ParseState::InAnswer => self.finalize_note()?,
            _ => return Err(ParseError::InvalidStateChange(self.line_num)),
        }

        self.state = self.state.reset()?;
        self.deck = None;

        let results = self.parsed.clone();
        self.parsed.clear();
        
        debug!("Found notes: {:?}", results);

        Ok(results)
    }
}

struct AnkiMarkdownHandler {
    parser: Parser,
}

impl AnkiMarkdownHandler {
    fn new(question_token: &str, answer_token: &str) -> Self {
        Self {
            parser: Parser::new(question_token, answer_token),
        }
    }

    fn default() -> Self {
        Self::new("Q: ", "A: ")
    }

    fn parse_file(&mut self, path: &PathBuf) -> Result<Vec<ParsedNote>, ParseError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let event = line?;
            self.parser.handle_event(&event)?;
        }

        self.parser.finalize()
    }
}

#[derive(Error, Debug)]
pub enum AnkiSyncError {
    #[error("Received API error: {0}")]
    ApiError(#[from] ApiError),
    #[error("Received parsing error: {0}")]
    ParseError(#[from] ParseError),
}

pub struct AnkiSync {
    anki_client: AnkiConnectClient,
    md_handler: AnkiMarkdownHandler,
}

impl AnkiSync {
    pub fn new() -> Self {
        Self {
            anki_client: AnkiConnectClient::default(),
            md_handler: AnkiMarkdownHandler::default(),
        }
    }

    pub async fn sync_file(&mut self, file: &PathBuf) -> Result<(), AnkiSyncError> {
        let parsed_notes = self.md_handler.parse_file(file)?;
        self.anki_client.add_notes(parsed_notes).await?;
        Ok(())
    }
}
