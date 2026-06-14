use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::collections::HashSet;

pub const BOARD_WIDTH: u16 = 960;
pub const BOARD_HEIGHT: u16 = 620;
const CARD_WIDTH: u16 = 86;
const CARD_HEIGHT: u16 = 122;
const INITIAL_HAND_SIZE: usize = 6;
const FIRST_NAMES: &str = include_str!("../data/first_names.txt");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: String,
    pub name: String,
    hand: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardCard {
    number: u8,
    x: u16,
    y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardState {
    deck: Vec<u8>,
    players: Vec<Player>,
    board_cards: Vec<BoardCard>,
    next_player_number: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardError {
    CardNotFound,
    InvalidMovePayload,
    InvalidNamePayload,
    PlayerNotFound,
}

impl BoardState {
    pub fn new() -> Self {
        Self::with_seed(current_seed())
    }

    pub fn with_seed(seed: u64) -> Self {
        let mut deck = (1..=101).collect::<Vec<_>>();
        shuffle_cards(&mut deck, seed);

        Self {
            deck,
            players: Vec::new(),
            board_cards: Vec::new(),
            next_player_number: 1,
        }
    }

    pub fn ensure_player(&mut self, player_id: Option<&str>) -> String {
        if let Some(player_id) = player_id {
            if self.players.iter().any(|player| player.id == player_id) {
                return player_id.to_string();
            }
        }

        self.create_player()
    }

    pub fn has_player(&self, player_id: &str) -> bool {
        self.players.iter().any(|player| player.id == player_id)
    }

    pub fn rename_player(&mut self, player_id: &str, name: &str) -> Result<(), BoardError> {
        let clean_name = clean_player_name(name).ok_or(BoardError::InvalidNamePayload)?;
        let player = self
            .players
            .iter_mut()
            .find(|player| player.id == player_id)
            .ok_or(BoardError::PlayerNotFound)?;

        player.name = clean_name;
        Ok(())
    }

    pub fn move_board_card(&mut self, card_id: &str, x: u16, y: u16) -> Result<(), BoardError> {
        let card_number = parse_card_id(card_id).ok_or(BoardError::CardNotFound)?;
        let card = self
            .board_cards
            .iter_mut()
            .find(|card| card.number == card_number)
            .ok_or(BoardError::CardNotFound)?;

        let (bounded_x, bounded_y) = bound_board_position(x, y);
        card.x = bounded_x;
        card.y = bounded_y;
        Ok(())
    }

    pub fn play_card_from_hand(
        &mut self,
        player_id: &str,
        card_id: &str,
        x: u16,
        y: u16,
    ) -> Result<(), BoardError> {
        let card_number = parse_card_id(card_id).ok_or(BoardError::CardNotFound)?;
        let player = self
            .players
            .iter_mut()
            .find(|player| player.id == player_id)
            .ok_or(BoardError::PlayerNotFound)?;
        let hand_index = player
            .hand
            .iter()
            .position(|number| *number == card_number)
            .ok_or(BoardError::CardNotFound)?;
        let card_number = player.hand.remove(hand_index);
        let (bounded_x, bounded_y) = bound_board_position(x, y);

        self.board_cards.push(BoardCard {
            number: card_number,
            x: bounded_x,
            y: bounded_y,
        });

        Ok(())
    }

    pub fn take_card_from_board(
        &mut self,
        player_id: &str,
        card_id: &str,
    ) -> Result<(), BoardError> {
        if !self.has_player(player_id) {
            return Err(BoardError::PlayerNotFound);
        }

        let card_number = parse_card_id(card_id).ok_or(BoardError::CardNotFound)?;
        let board_index = self
            .board_cards
            .iter()
            .position(|card| card.number == card_number)
            .ok_or(BoardError::CardNotFound)?;
        let card = self.board_cards.remove(board_index);
        let player = self
            .players
            .iter_mut()
            .find(|player| player.id == player_id)
            .ok_or(BoardError::PlayerNotFound)?;

        player.hand.push(card.number);
        player.hand.sort_unstable();
        Ok(())
    }

    pub fn view_for(&self, player_id: &str) -> String {
        let player = self.players.iter().find(|player| player.id == player_id);
        let me_json = player
            .map(player_to_private_json)
            .unwrap_or_else(|| "null".to_string());
        let players_json = self
            .players
            .iter()
            .map(|player| player_to_public_json(player, player.id == player_id))
            .collect::<Vec<_>>()
            .join(",");
        let board_cards_json = self
            .board_cards
            .iter()
            .map(board_card_to_json)
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"width\":{},\"height\":{},\"me\":{},\"players\":[{}],\"boardCards\":[{}]}}",
            BOARD_WIDTH, BOARD_HEIGHT, me_json, players_json, board_cards_json
        )
    }

    #[cfg(test)]
    pub fn card_ownership_is_consistent(&self) -> bool {
        let mut seen = HashSet::new();

        for card in &self.deck {
            if !seen.insert(*card) {
                return false;
            }
        }

        for player in &self.players {
            for card in &player.hand {
                if !seen.insert(*card) {
                    return false;
                }
            }
        }

        for card in &self.board_cards {
            if !seen.insert(card.number) {
                return false;
            }
        }

        seen.len() == 101
    }

    fn create_player(&mut self) -> String {
        let player_number = self.next_player_number;
        self.next_player_number += 1;

        let player_id = format!("player-{player_number}");
        let name = self.default_player_name(player_number);
        let hand = self.deal_initial_hand();

        self.players.push(Player {
            id: player_id.clone(),
            name,
            hand,
        });

        player_id
    }

    fn default_player_name(&self, player_number: u64) -> String {
        let names = first_names();
        let index = ((player_number - 1) as usize) % names.len();
        let suffix = ((player_number - 1) / names.len() as u64) + 1;

        if suffix == 1 {
            names[index].to_string()
        } else {
            format!("{} {}", names[index], suffix)
        }
    }

    fn deal_initial_hand(&mut self) -> Vec<u8> {
        let mut hand = Vec::new();

        for _ in 0..INITIAL_HAND_SIZE {
            if let Some(card) = self.deck.pop() {
                hand.push(card);
            }
        }

        hand.sort_unstable();
        hand
    }
}

pub fn parse_move_payload(payload: &str) -> Result<(u16, u16), BoardError> {
    let x = extract_u16_json_field(payload, "x").ok_or(BoardError::InvalidMovePayload)?;
    let y = extract_u16_json_field(payload, "y").ok_or(BoardError::InvalidMovePayload)?;

    Ok((x, y))
}

pub fn parse_name_payload(payload: &str) -> Result<String, BoardError> {
    extract_string_json_field(payload, "name")
        .and_then(|name| clean_player_name(&name))
        .ok_or(BoardError::InvalidNamePayload)
}

fn current_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x7175_6970_7265_6e64)
}

fn shuffle_cards(cards: &mut [u8], mut seed: u64) {
    for index in (1..cards.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let swap_index = (seed as usize) % (index + 1);
        cards.swap(index, swap_index);
    }
}

fn first_names() -> Vec<&'static str> {
    FIRST_NAMES
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn parse_card_id(card_id: &str) -> Option<u8> {
    let number = card_id.strip_prefix("card-")?.parse::<u8>().ok()?;
    (1..=101).contains(&number).then_some(number)
}

fn bound_board_position(x: u16, y: u16) -> (u16, u16) {
    let max_x = BOARD_WIDTH.saturating_sub(CARD_WIDTH);
    let max_y = BOARD_HEIGHT.saturating_sub(CARD_HEIGHT);

    (x.min(max_x), y.min(max_y))
}

fn clean_player_name(name: &str) -> Option<String> {
    let clean_name = name.trim();

    if clean_name.is_empty() || clean_name.chars().count() > 32 {
        return None;
    }

    Some(clean_name.to_string())
}

fn extract_u16_json_field(payload: &str, field_name: &str) -> Option<u16> {
    let field_marker = format!("\"{}\"", field_name);
    let field_index = payload.find(&field_marker)?;
    let after_field = &payload[field_index + field_marker.len()..];
    let colon_index = after_field.find(':')?;
    let after_colon = after_field[colon_index + 1..].trim_start();
    let number_text = after_colon
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();

    if number_text.is_empty() {
        return None;
    }

    number_text.parse::<u16>().ok()
}

fn extract_string_json_field(payload: &str, field_name: &str) -> Option<String> {
    let field_marker = format!("\"{}\"", field_name);
    let field_index = payload.find(&field_marker)?;
    let after_field = &payload[field_index + field_marker.len()..];
    let colon_index = after_field.find(':')?;
    let after_colon = after_field[colon_index + 1..].trim_start();
    let quoted_value = after_colon.strip_prefix('"')?;
    let mut value = String::new();
    let mut escaped = false;

    for character in quoted_value.chars() {
        if escaped {
            value.push(match character {
                '"' => '"',
                '\\' => '\\',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}

fn player_to_private_json(player: &Player) -> String {
    let hand_json = player
        .hand
        .iter()
        .map(|number| card_number_to_json(*number))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"id\":\"{}\",\"name\":\"{}\",\"hand\":[{}]}}",
        escape_json_string(&player.id),
        escape_json_string(&player.name),
        hand_json
    )
}

fn player_to_public_json(player: &Player, is_current_player: bool) -> String {
    let hidden_cards_json = (0..player.hand.len())
        .map(|_| "{\"faceDown\":true}".to_string())
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"id\":\"{}\",\"name\":\"{}\",\"isCurrentPlayer\":{},\"handCount\":{},\"hiddenHand\":[{}]}}",
        escape_json_string(&player.id),
        escape_json_string(&player.name),
        is_current_player,
        player.hand.len(),
        hidden_cards_json
    )
}

fn board_card_to_json(card: &BoardCard) -> String {
    format!(
        "{{\"id\":\"card-{}\",\"number\":{},\"label\":\"{}\",\"x\":{},\"y\":{},\"color\":\"{}\"}}",
        card.number,
        card.number,
        card.number,
        card.x,
        card.y,
        card_color(card.number)
    )
}

fn card_number_to_json(number: u8) -> String {
    format!(
        "{{\"id\":\"card-{number}\",\"number\":{number},\"label\":\"{number}\",\"color\":\"{}\"}}",
        card_color(number)
    )
}

fn card_color(number: u8) -> &'static str {
    const COLORS: [&str; 10] = [
        "#f6d365", "#9be7c7", "#93c5fd", "#fca5a5", "#d8b4fe", "#fde68a", "#67e8f9", "#f9a8d4",
        "#c4b5fd", "#bef264",
    ];

    COLORS[(number as usize - 1) % COLORS.len()]
}

fn escape_json_string(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_players_receive_private_unique_hands() {
        let mut board = BoardState::with_seed(42);

        let first_player = board.ensure_player(None);
        let second_player = board.ensure_player(None);

        assert!(board.card_ownership_is_consistent());
        assert_ne!(first_player, second_player);
        assert_eq!(board.players[0].hand.len(), 6);
        assert_eq!(board.players[1].hand.len(), 6);
    }

    #[test]
    fn private_view_shows_current_hand_but_only_hidden_opponent_hand() {
        let mut board = BoardState::with_seed(42);
        let first_player = board.ensure_player(None);
        let _second_player = board.ensure_player(None);

        let view = board.view_for(&first_player);

        assert!(view.contains("\"hand\":[{\"id\":\"card-"));
        assert!(view.contains("\"hiddenHand\":[{\"faceDown\":true}"));
        assert!(view.contains("\"handCount\":6"));
    }

    #[test]
    fn player_can_only_play_cards_from_their_own_hand() {
        let mut board = BoardState::with_seed(42);
        let first_player = board.ensure_player(None);
        let second_player = board.ensure_player(None);
        let second_player_card = board.players[1].hand[0];

        assert_eq!(
            board.play_card_from_hand(&first_player, &format!("card-{second_player_card}"), 20, 30),
            Err(BoardError::CardNotFound)
        );

        assert!(board
            .play_card_from_hand(
                &second_player,
                &format!("card-{second_player_card}"),
                20,
                30
            )
            .is_ok());
    }

    #[test]
    fn board_cards_can_be_taken_into_current_player_hand() {
        let mut board = BoardState::with_seed(42);
        let first_player = board.ensure_player(None);
        let card = board.players[0].hand[0];

        board
            .play_card_from_hand(&first_player, &format!("card-{card}"), 20, 30)
            .unwrap();
        board
            .take_card_from_board(&first_player, &format!("card-{card}"))
            .unwrap();

        assert!(board.players[0].hand.contains(&card));
        assert!(board.board_cards.is_empty());
        assert!(board.card_ownership_is_consistent());
    }

    #[test]
    fn move_payload_accepts_whitespace() {
        assert_eq!(
            parse_move_payload(r#"{ "y" : 24, "x" : 12 }"#),
            Ok((12, 24))
        );
    }

    #[test]
    fn name_payload_is_trimmed() {
        assert_eq!(
            parse_name_payload(r#"{ "name" : " Camille " }"#),
            Ok("Camille".to_string())
        );
    }

    #[test]
    fn moved_cards_are_kept_inside_the_board() {
        let mut board = BoardState::with_seed(42);
        let player = board.ensure_player(None);
        let card = board.players[0].hand[0];

        board
            .play_card_from_hand(&player, &format!("card-{card}"), 2_000, 2_000)
            .unwrap();

        let json = board.view_for(&player);
        assert!(json.contains("\"x\":874"));
        assert!(json.contains("\"y\":498"));
    }
}
