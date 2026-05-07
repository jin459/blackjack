# Blackjack Workspace — Tài liệu kỹ thuật

> Hướng dẫn chi tiết: kiến trúc, từng dòng code, và lý do đằng sau mỗi quyết định thiết kế.

---

## Mục lục

1. [Tổng quan & Mục tiêu](#1-tổng-quan--mục-tiêu)
2. [Kiến trúc tổng thể](#2-kiến-trúc-tổng-thể)
3. [Workspace Rust](#3-workspace-rust)
4. [Domain Layer](#4-domain-layer)
5. [Application Layer](#5-application-layer)
6. [Infrastructure Layer](#6-infrastructure-layer)
7. [API Layer](#7-api-layer)
8. [Observability](#8-observability)
9. [Chạy project](#9-chạy-project)
10. [Luồng game hoàn chỉnh](#10-luồng-game-hoàn-chỉnh)

---

## 1. Tổng quan & Mục tiêu

### Chúng ta đang build gì?

Một game Blackjack server với:
- **REST API** để điều khiển game (tạo bàn, đặt cược, hit, stand...)
- **WebSocket** để push event realtime tới client (card được chia, player bust...)
- **Cache** trạng thái game trên Redis để server không giữ state trong bộ nhớ
- **Message Queue** (Redis Streams) để broadcast domain events
- **Distributed tracing** với OpenTelemetry → Jaeger
- **Metrics** với Prometheus → Grafana

### Tại sao DDD (Domain-Driven Design)?

DDD giúp chúng ta:

| Vấn đề thông thường | Giải pháp DDD |
|---|---|
| Business logic rải rác khắp nơi (controller, service, DB) | Tập trung trong `domain/` |
| Khó test vì phụ thuộc I/O | `domain/` pure Rust, zero I/O, test nhanh |
| Thay Redis bằng PostgreSQL phải sửa nhiều nơi | Chỉ sửa `infrastructure/`, domain không đổi |
| Khó hiểu ý nghĩa của code | Code nói ngôn ngữ của domain: `game.hit(player)` |

---

## 2. Kiến trúc tổng thể

### Hexagonal Architecture (Ports & Adapters)

```
┌─────────────────────────────────────────────────────────────┐
│                        API Layer                             │
│  (Actix-web, WebSocket, HTTP handlers, OTel middleware)     │
└────────────────────────────┬────────────────────────────────┘
                             │ gọi
┌────────────────────────────▼────────────────────────────────┐
│                   Application Layer                          │
│  (GameCommandService, GameQueryService, DTOs, CQRS)         │
└──────────┬──────────────────────────────────┬───────────────┘
           │ depends on (trait)               │ depends on (trait)
┌──────────▼──────────┐          ┌────────────▼───────────────┐
│    Domain Layer      │          │    Infrastructure Layer     │
│  (Game, Player,      │◄─impl───│  (RedisGameRepository,     │
│   Card, Hand, Bet,   │         │   RedisEventPublisher,     │
│   Repository traits) │         │   Redis Streams)           │
└──────────────────────┘          └────────────────────────────┘
```

**Quy tắc phụ thuộc (Dependency Rule):**
- Mũi tên chỉ VÀO domain, không chỉ ra
- `domain/` không biết Redis, Actix-web, hay bất kỳ framework nào tồn tại
- Nếu cần đổi Redis → PostgreSQL: chỉ viết lại `infrastructure/`

### Tại sao quan trọng?

```rust
// domain/src/repository.rs - đây là PORT (interface)
#[async_trait]
pub trait GameRepository: Send + Sync {
    async fn save(&self, game: &Game) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: GameId) -> Result<Option<Game>, DomainError>;
}

// infrastructure/src/redis_cache.rs - đây là ADAPTER (implementation)
pub struct RedisGameRepository { conn: ConnectionManager }

#[async_trait]
impl GameRepository for RedisGameRepository { ... }
```

Domain chỉ biết "tôi cần lưu game" (trait). Infrastructure quyết định "lưu vào Redis" (impl). Domain không import redis.

---

## 3. Workspace Rust

### Cargo Workspace là gì?

Một workspace cho phép nhiều crate trong cùng một repository, chia sẻ dependencies và `target/` directory.

```toml
# Cargo.toml (root)
[workspace]
members = [
    "crates/domain",
    "crates/application",
    "crates/infrastructure",
    "crates/api",
]
resolver = "2"
```

**`resolver = "2"`**: dùng feature resolver thế hệ 2 của Cargo — tránh conflicts khi các crate con dùng cùng dep với features khác nhau.

### Workspace Dependencies

```toml
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
# ...
```

**Tại sao khai báo ở đây?**  
Mỗi crate con dùng `{ workspace = true }` thay vì lặp lại version. Khi upgrade, chỉ sửa 1 chỗ. Đảm bảo toàn workspace dùng cùng version (tránh diamond dependency conflict).

### Dependency Graph giữa các crate

```
api
 ├── application
 │    └── domain
 ├── infrastructure
 │    ├── domain
 │    └── application
 └── domain
```

`domain` không depend vào ai. `application` chỉ depend `domain`. `infrastructure` implement traits của `domain`. `api` orchestrate tất cả.

---

## 4. Domain Layer

**Vị trí:** `crates/domain/src/`

### 4.1 Value Objects vs Entities

**Value Object** — không có identity, so sánh bằng giá trị, immutable:
- `Card` (♥A = ♥A bất kể "đây là lá bài nào")
- `Hand` (tập hợp cards)
- `Bet` (số tiền cược)
- `Chips` (số chip)

**Entity** — có identity riêng, mutable state:
- `Player` (identified by `PlayerId`)
- `Game` (identified by `GameId`)

**Tại sao phân biệt?**  
Hai lá bài cùng loại là như nhau (value object). Nhưng hai player cùng tên là hai người khác nhau (entity).

### 4.2 `ids.rs` — Newtype Pattern

```rust
macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            pub fn inner(self) -> Uuid { self.0 }
        }
    };
}

uuid_id!(GameId);
uuid_id!(PlayerId);
```

**Tại sao không dùng `Uuid` trực tiếp?**

```rust
// Không an toàn — compiler không bắt được lỗi:
fn join_game(game_id: Uuid, player_id: Uuid) { ... }
join_game(player_uuid, game_uuid); // BUG: đổi ngược — compiler OK!

// An toàn với newtype:
fn join_game(game_id: GameId, player_id: PlayerId) { ... }
join_game(player_id, game_id); // COMPILE ERROR!
```

Newtype tạo ra type-level safety: compiler bắt được lỗi truyền nhầm argument.

**Tại sao dùng macro?**  
`GameId` và `PlayerId` có implementation giống hệt nhau. Macro tránh copy-paste và đảm bảo consistency.

### 4.3 `card.rs` — Suit, Rank, Card

```rust
pub enum Suit { Hearts, Diamonds, Clubs, Spades }

pub enum Rank {
    Ace, Two, Three, ..., King
}

impl Rank {
    pub fn base_value(self) -> u8 {
        match self {
            Rank::Ace => 11,   // Ace mặc định = 11; Hand điều chỉnh nếu cần
            Rank::Ten | Rank::Jack | Rank::Queen | Rank::King => 10,
            // ...
        }
    }
}
```

**Tại sao Ace = 11 ở Rank level?**  
Ace có thể là 1 hoặc 11 tùy context. Nhưng logic "khi nào Ace = 1" là trách nhiệm của `Hand`, không phải `Card`. `Rank` chỉ cần biết "giá trị mặc định" (soft value). Tách biệt concern rõ ràng.

### 4.4 `hand.rs` — Scoring Algorithm

```rust
pub fn score(&self) -> u8 {
    let mut total: u8 = 0;
    let mut soft_aces: u8 = 0;

    for card in &self.cards {
        if card.is_ace() {
            soft_aces += 1;
            total = total.saturating_add(11); // Ace = 11 trước
        } else {
            total = total.saturating_add(card.base_value());
        }
    }

    // Nếu tổng > 21, chuyển Ace từ 11 → 1 (bớt 10)
    while total > 21 && soft_aces > 0 {
        total -= 10;
        soft_aces -= 1;
    }

    total
}
```

**Thuật toán**: Luôn count Ace = 11 trước. Nếu bust, convert từng Ace xuống 1 (trừ 10) cho đến khi không bust hoặc hết Ace.

**Tại sao `saturating_add`?** Tránh panic do integer overflow trong Rust. Nếu tổng vượt `u8::MAX` (255), nó giữ nguyên 255 thay vì wrap around hay panic. Trong blackjack không thể xảy ra (max ~8 Aces = 88) nhưng defensive programming.

```rust
pub fn outcome(&self) -> HandOutcome {
    let score = self.score();
    if score > 21 { return HandOutcome::Bust; }
    // Natural blackjack: đúng 2 lá, tổng = 21
    if self.cards.len() == 2 && score == 21 { return HandOutcome::Blackjack; }
    if self.has_soft_ace() { HandOutcome::Soft(score) }
    else { HandOutcome::Hard(score) }
}
```

**Tại sao phân biệt `Soft` và `Hard`?**  
Dealer rules trong blackjack: "stand on soft 17, hit on hard 17". Soft 17 = Ace + 6 (Ace vẫn là 11). Hard 17 = 10 + 7 (không có soft Ace). Nếu không phân biệt, không implement được dealer rules đúng.

### 4.5 `bet.rs` — Chips và Bet

```rust
pub struct Chips(u32);  // Newtype: không thể nhầm với u32 thông thường

impl Chips {
    pub const ZERO: Chips = Chips(0);

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}

impl Add for Chips {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

pub struct Bet(Chips);  // Bet != Chips: Bet luôn > 0

impl Bet {
    pub fn new(amount: u32) -> Result<Self, DomainError> {
        if amount == 0 {
            return Err(DomainError::InvalidBet("bet must be > 0".into()));
        }
        Ok(Self(Chips::new(amount)))
    }
}
```

**Tại sao tách `Chips` và `Bet`?**  
`Chips` là "số tiền trong ví" — có thể bằng 0. `Bet` là "số tiền đặt cược" — bao giờ cũng > 0. Type system enforce invariant này tại compile time: bạn không thể tạo `Bet(0)`.

**`checked_sub` vs regular subtraction?**  
`Chips` không thể âm. `checked_sub` trả về `Option<Chips>` để caller xử lý trường hợp không đủ tiền thay vì panic.

### 4.6 `game.rs` — Aggregate Root

`Game` là trái tim của domain. Nó là **Aggregate Root**: entry point duy nhất để thay đổi state của một game.

#### State Machine

```
WaitingForPlayers
    │ open_betting()
    ▼
WaitingForBets
    │ deal() — khi tất cả đã bet
    ▼
PlayerTurn { seat_index: 0 }
    │ hit() / stand() per player
    │ (seat_index tăng dần, skip Blackjack/Bust)
    ▼
DealerTurn
    │ play_dealer()
    ▼
Settled
```

**Tại sao dùng state machine?**  
Blackjack có nhiều rule liên quan đến thứ tự. State machine ngăn chặn:
- Gọi `hit()` khi chưa `deal()`
- Gọi `deal()` khi chưa đủ người bet
- Gọi `play_dealer()` khi còn player chưa xong

Mỗi command bắt đầu bằng `if !matches!(self.status, ExpectedStatus)`:
```rust
pub fn deal(&mut self) -> Result<(), DomainError> {
    if !matches!(self.status, GameStatus::WaitingForBets) {
        return Err(DomainError::InvalidAction("not in betting phase".into()));
    }
    // ...
}
```

#### Collect-then-Dispatch Pattern

```rust
pub struct Game {
    // ...
    #[serde(skip)]           // Events không được persist vào Redis
    pending_events: Vec<DomainEvent>,
}

// Sau mỗi mutation:
self.pending_events.push(DomainEvent::CardDealt { ... });

// Application layer drain sau khi xong:
pub fn drain_events(&mut self) -> Vec<DomainEvent> {
    std::mem::take(&mut self.pending_events)  // lấy và clear
}
```

**Tại sao không publish event trực tiếp trong domain?**  
- Domain không biết Redis hay message queue tồn tại
- Cho phép transaction: save aggregate và publish events cùng lúc (hoặc không làm gì nếu save fail)
- Dễ test: có thể inspect events mà không cần mock publisher

```rust
// Trong application layer:
async fn save_and_publish(&self, game: &mut Game) -> Result<(), AppError> {
    let events = game.drain_events();   // lấy events
    self.game_repo.save(game).await?;   // lưu aggregate
    if !events.is_empty() {
        self.event_pub.publish_all(events).await?; // publish
    }
    Ok(())
}
```

#### Shoe Shuffle — XorShift64

```rust
fn build_shoe(num_decks: usize) -> Vec<Card> {
    let mut shoe = Vec::with_capacity(52 * num_decks);
    for _ in 0..num_decks {
        for suit in suits { for rank in ranks { shoe.push(Card::new(suit, rank)); } }
    }
    // Fisher-Yates shuffle với XorShift64
    let mut s = uuid::Uuid::new_v4().as_u128() as u64 | 1; // seed ngẫu nhiên
    for i in (1..shoe.len()).rev() {
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; // XorShift steps
        let j = (s as usize) % (i + 1);
        shoe.swap(i, j);
    }
    shoe
}
```

**Tại sao XorShift?**  
Không có external dependency, nhanh, đủ ngẫu nhiên cho game. Seed từ `Uuid::new_v4()` (CSPRNG của OS) nên mỗi lần khác nhau. Production game cần CSPRNG đầy đủ nhưng đây là prototype.

**`| 1` ở cuối?**  
XorShift yêu cầu seed != 0. OR với 1 đảm bảo điều đó dù UUID có thể generate 0 (cực hiếm).

### 4.7 `events.rs` — Domain Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    GameStarted { game_id: GameId },
    CardDealt { game_id: GameId, player_id: Option<PlayerId>, card: Card },
    PlayerStood { game_id: GameId, player_id: PlayerId },
    PlayerBust { game_id: GameId, player_id: PlayerId },
    GameEnded { game_id: GameId, dealer_score: u8 },
}
```

**`#[serde(tag = "type")]`**: serialize thành JSON với field `"type"` chứa tên variant:
```json
{ "type": "card_dealt", "game_id": "...", "player_id": "...", "card": {...} }
```

Client nhận JSON qua WebSocket biết ngay đây là event gì mà không cần parse structure.

**`player_id: Option<PlayerId>`** trong `CardDealt`: `None` = card của dealer. Dealer không có PlayerId.

### 4.8 `repository.rs` — Ports

```rust
#[async_trait]
pub trait GameRepository: Send + Sync {
    async fn save(&self, game: &Game) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: GameId) -> Result<Option<Game>, DomainError>;
    async fn list_active(&self) -> Result<Vec<Game>, DomainError>;
    async fn delete(&self, id: GameId) -> Result<(), DomainError>;
}
```

**`Send + Sync`**: trait object (`Arc<dyn GameRepository>`) cần be shareable across threads. Actix-web chạy trên Tokio multi-thread runtime.

**`async_trait`**: Rust không hỗ trợ async fn trong trait natively (stable). Macro `#[async_trait]` transform thành `fn(...) -> Pin<Box<dyn Future>>` internally.

**`find_by_id` trả `Option<Game>` thay vì `Result<Game>`?**  
"Not found" không phải lỗi hệ thống. Caller (application layer) quyết định xem không tìm thấy có phải là lỗi không.

### 4.9 `services.rs` — Domain Service

```rust
pub struct BlackjackRules;

impl BlackjackRules {
    pub fn calculate_payouts(game: &Game) -> Vec<Payout> {
        // Pure function: không I/O, không side effect
        game.seats().iter().map(|seat| {
            let bet_amount = seat.bet.map(|b| b.amount()).unwrap_or(0);
            let (outcome, return_amount) = match &seat.status {
                SeatStatus::Blackjack => {
                    let profit = (bet_amount * 3) / 2; // 3:2 payout
                    (RoundOutcome::PlayerBlackjack, bet_amount + profit)
                }
                SeatStatus::Bust => (RoundOutcome::PlayerLose, 0),
                SeatStatus::Stood => {
                    if dealer_bust || player_score > dealer_score {
                        (RoundOutcome::PlayerWin, bet_amount * 2)
                    } else if player_score == dealer_score {
                        (RoundOutcome::Push, bet_amount) // hoàn tiền
                    } else {
                        (RoundOutcome::PlayerLose, 0) // mất cược
                    }
                }
                // ...
            };
            Payout { player_id: seat.player_id, bet_amount, outcome, return_amount }
        }).collect()
    }
}
```

**Tại sao `return_amount` thay vì `delta`?**  
`return_amount` là số chip được credit lại. Logic đơn giản: khi đặt cược, deduct ngay. Khi kết thúc, credit lại theo result:
- Win (bet=100): return 200 (100 stake + 100 profit)
- Blackjack (bet=100): return 250 (100 stake + 150 profit = 3:2)
- Push (bet=100): return 100 (hoàn stake)
- Loss: return 0 (stake đã mất)

**Tại sao `Domain Service` thay vì method trên `Game`?**  
Payout calculation liên quan đến nhiều thứ (seat status, dealer score, bet amount). Đặt trong `Game` sẽ làm aggregate quá lớn. Domain Service = logic không thuộc về một entity cụ thể.

---

## 5. Application Layer

**Vị trí:** `crates/application/src/`

### 5.1 CQRS — Command Query Responsibility Segregation

```
Commands (write)                   Queries (read)
──────────────────                 ──────────────
CreateGame                         GetGameState
JoinGame                           GetPlayer
PlaceBet                           ListActiveGames
Deal
Hit
Stand
PlayDealer
Settle
```

**Tại sao tách?**  
- Commands thay đổi state → cần validate, emit events, save
- Queries chỉ đọc → không cần lock, có thể optimize riêng (read model, cache)
- Mỗi loại có lifecycle khác nhau

### 5.2 `commands.rs`

```rust
#[derive(Debug)]
pub struct PlaceBetCommand {
    pub game_id: GameId,
    pub player_id: PlayerId,
    pub amount: u32,
}
```

**Tại sao `#[derive(Debug)]`?**  
`#[instrument]` macro của `tracing` crate cần `Debug` để log command details vào span. Bỏ `Debug` → compile error.

**Command = plain struct**, không có method. Data carrier only.

### 5.3 `handlers.rs` — GameCommandService

```rust
pub struct GameCommandService {
    game_repo: Arc<dyn GameRepository>,
    player_repo: Arc<dyn PlayerRepository>,
    event_pub: Arc<dyn EventPublisher>,
}
```

**`Arc<dyn Trait>`**: shared ownership của trait object. `Arc` = Atomic Reference Counting, thread-safe. `dyn Trait` = dynamic dispatch (runtime polymorphism). Cho phép inject different implementations (Redis, in-memory for tests...).

#### Instrument Macro

```rust
#[instrument(skip(self), fields(game.id = %cmd.game_id, player.id = %cmd.player_id, bet = cmd.amount))]
pub async fn place_bet(&self, cmd: PlaceBetCommand) -> Result<(), AppError> {
```

`#[instrument]`:
- Tạo một **tracing span** bao quanh function
- `skip(self)`: không log `self` (quá dài)
- `fields(...)`: thêm semantic attributes vào span
- Span tự động close khi function return
- OTel exporter gửi span lên Jaeger

Kết quả trong Jaeger: mỗi `place_bet` call có span với `game.id`, `player.id`, `bet`, duration, và nested spans từ Redis calls.

#### Pattern: Load → Mutate → Save → Publish

```rust
pub async fn place_bet(&self, cmd: PlaceBetCommand) -> Result<(), AppError> {
    let bet = Bet::new(cmd.amount)?;          // 1. Validate input

    let mut player = self.load_player(cmd.player_id).await?;  // 2. Load
    player.deduct(bet.chips())?;              // 3. Mutate player

    let mut game = self.load_game(cmd.game_id).await?;         // 4. Load
    game.place_bet(cmd.player_id, bet)?;      // 5. Mutate game (domain rule)

    self.player_repo.save(&player).await?;    // 6. Save player
    self.save_and_publish(&mut game).await    // 7. Save game + publish events
}
```

**Tại sao load player trước game?**  
Validate player tồn tại và có đủ chip trước khi load game. Fail fast: không load game nếu player không hợp lệ.

**Vấn đề: không có transaction!**  
Player save thành công nhưng game save fail → inconsistent state. Trong production cần Two-Phase Commit hoặc Saga pattern. Đây là trade-off chấp nhận được ở prototype stage.

### 5.4 `queries.rs` — DTOs và Mapping

```rust
pub struct GameStateDto {
    pub id: String,
    pub status: String,
    pub dealer_hand: HandDto,
    pub seats: Vec<SeatDto>,
}
```

**Tại sao trả DTO thay vì domain object?**

1. **Decoupling**: API layer không phụ thuộc domain types
2. **Versioning**: có thể thay đổi domain model mà không break API
3. **Projection**: DTO có thể chứa computed fields, exclude sensitive data
4. **Serialization control**: domain types có thể không serializable hoặc serialize khác format mong muốn

```rust
pub fn game_to_state_dto(g: &Game) -> GameStateDto {
    GameStateDto {
        id: g.id().to_string(),
        status: game_status_str(g.status()), // enum → string cho JSON
        dealer_hand: hand_to_dto(g.dealer_hand()),
        seats: g.seats().iter().map(|s| SeatDto { ... }).collect(),
    }
}
```

Mapping function pure: không I/O, dễ test, dễ đọc.

---

## 6. Infrastructure Layer

**Vị trí:** `crates/infrastructure/src/`

### 6.1 Redis Schema

```
Key                      Type    TTL     Mục đích
──────────────────────── ─────── ─────── ────────────────────────
bj:game:{uuid}           STRING  1 hour  Game state (JSON)
bj:games:active          SET     -       Index: active game IDs
bj:player:{uuid}         STRING  -       Player state (JSON)
bj:players:all           SET     -       Index: all player IDs
bj:events                STREAM  -       Domain events
```

**Tại sao JSON thay vì binary?**  
Dễ debug (redis-cli có thể đọc), không cần schema migration khi thêm field mới.

**Tại sao TTL 1 giờ cho game?**  
Game abandoned (không ai play) tự động expire, không tốn bộ nhớ vô ích.

**Tại sao cần `bj:games:active` SET?**  
Redis không có "list all keys matching pattern" hiệu quả. `KEYS bj:game:*` block Redis thread trong production. Dùng SET để maintain index riêng.

### 6.2 `redis_cache.rs` — Repository Implementation

```rust
pub struct RedisGameRepository {
    conn: ConnectionManager,
}
```

**`ConnectionManager`**: connection pool tự động reconnect. Cheap to clone — mỗi clone chỉ clone handle, không tạo connection mới.

```rust
async fn save(&self, game: &Game) -> Result<(), DomainError> {
    let mut c = self.conn.clone(); // lấy mutable handle
    let key = game_key(game.id()); // "bj:game:{uuid}"
    let json = serde_json::to_string(game).map_err(repo_err)?;

    let _: () = c.set_ex(&key, &json, GAME_TTL_SECS).await.map_err(repo_err)?;

    // Cập nhật index
    if matches!(game.status(), GameStatus::Settled) {
        let _: i64 = c.srem(ACTIVE_GAMES_KEY, &id_str).await.map_err(repo_err)?;
    } else {
        let _: i64 = c.sadd(ACTIVE_GAMES_KEY, &id_str).await.map_err(repo_err)?;
    }
    Ok(())
}
```

**`repo_err` helper:**
```rust
fn repo_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Repository(e.to_string())
}
```

Convert bất kỳ error nào thành `DomainError::Repository`. Domain layer chỉ thấy một loại error, không biết detail của Redis.

**`let _: () = ...`**: Redis commands trả generic `RV: FromRedisValue`. Annotate `()` để Rust biết ta không care về return value (và không cần borrow sau đó).

### 6.3 `redis_streams.rs` — Event Publisher

```rust
pub async fn publish_all(&self, events: Vec<DomainEvent>) -> Result<(), DomainError> {
    let mut c = self.conn.clone();
    for event in events {
        let payload = serde_json::to_string(&event).map_err(repo_err)?;
        let stream_id: String = redis::cmd("XADD")
            .arg("bj:events")
            .arg("*")          // auto-generate ID (timestamp-based)
            .arg("event_type").arg(event_type_name(&event))
            .arg("game_id").arg(event.game_id().to_string())
            .arg("payload").arg(&payload)
            .query_async(&mut c)
            .await
            .map_err(repo_err)?;
        tracing::debug!(stream_id, "event published");
    }
    Ok(())
}
```

**Tại sao Redis Streams thay vì Pub/Sub?**

| | Redis Pub/Sub | Redis Streams |
|---|---|---|
| Persistence | Không | Có (lưu trong Redis) |
| Replay | Không | Có (XREAD với ID cũ) |
| Consumer groups | Không | Có |
| Audit trail | Không | Có |

Streams cho phép WebSocket server đọc lại events nếu bị disconnect tạm thời.

**`"*"` trong XADD**: Redis tự generate ID dạng `<timestamp_ms>-<sequence>`. ID monotonically increasing → có thể dùng để paginate.

**Raw command thay vì high-level API:**  
`redis::cmd("XADD")...` rõ ràng hơn, portable qua redis crate versions, dễ debug khi cần.

### 6.4 `repositories.rs` — Factory Pattern

```rust
pub struct Repos {
    pub games: Arc<dyn GameRepository>,
    pub players: Arc<dyn PlayerRepository>,
    pub events: Arc<dyn EventPublisher>,
}

pub fn build_repos(conn: ConnectionManager) -> Repos {
    Repos {
        games: Arc::new(RedisGameRepository::new(conn.clone())),
        players: Arc::new(RedisPlayerRepository::new(conn.clone())),
        events: Arc::new(RedisEventPublisher::new(conn)),
    }
}
```

**Factory pattern**: tạo tất cả dependencies từ một `ConnectionManager`. Caller (main.rs) chỉ cần gọi `build_repos(conn)` thay vì tự wire từng thứ.

**Tại sao `Arc<dyn Trait>` thay vì concrete type?**  
`GameCommandService` nhận `Arc<dyn GameRepository>`. Khi test, inject `Arc<InMemoryGameRepository>` thay vì `Arc<RedisGameRepository>` — không cần Redis thật.

---

## 7. API Layer

**Vị trí:** `crates/api/src/`

### 7.1 `config.rs` — Configuration

```rust
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub redis_url: String,
    pub otel_exporter_otlp_endpoint: String,
    pub otel_service_name: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Config::builder()
            .set_default("server_port", 8080)?
            // ...
            .add_source(Environment::default()) // đọc từ env vars
            .build()?
            .try_deserialize()
    }
}
```

**`config` crate**: đọc config từ files, env vars, command line args theo thứ tự ưu tiên. Default values bị override bởi env vars. Phù hợp với 12-Factor App methodology.

**`dotenvy::dotenv().ok()`** trong main: load `.env` file vào env vars. `.ok()` ignore lỗi nếu file không tồn tại (production không cần `.env`).

### 7.2 `error.rs` — Error Handling Chain

```
DomainError                     (domain/)
    │ From impl
    ▼
AppError                        (application/)
    │ From impl (via ApiError)
    ▼
ApiError impl ResponseError     (api/)
    │
    ▼
HTTP Response (400/404/409/500)
```

```rust
// api/src/error.rs
impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match &self.0 {
            AppError::NotFound(_)    => HttpResponse::NotFound().json(body),
            AppError::Conflict(_)    => HttpResponse::Conflict().json(body),
            AppError::InvalidInput(_)=> HttpResponse::BadRequest().json(body),
            AppError::Internal(_)    => HttpResponse::InternalServerError().json(body),
        }
    }
}
```

**Tại sao `ApiError` wrapper thay vì implement `ResponseError` cho `AppError` trực tiếp?**  
Orphan rule trong Rust: không thể implement external trait (`ResponseError`) cho external type (`AppError` từ crate khác). `ApiError` là newtype wrapper trong crate này → được phép.

**Kết quả trong route handler:**
```rust
pub async fn place_bet(state: web::Data<AppState>, ...) -> Result<HttpResponse, ApiError> {
    state.cmd.place_bet(cmd).await?; // AppError auto-converts to ApiError via From
    Ok(HttpResponse::Ok().finish())
}
```

`?` operator tự động gọi `ApiError::from(app_error)` nhờ `impl From<AppError> for ApiError`.

### 7.3 Route Handlers

```rust
#[post("/games/{id}/bet")]
pub async fn place_bet(
    state: web::Data<AppState>,    // shared state (Arc internally)
    path: web::Path<Uuid>,         // /games/{id} → Uuid
    body: web::Json<BetBody>,      // request body JSON
) -> Result<HttpResponse, ApiError> {
    state.cmd.place_bet(PlaceBetCommand {
        game_id: GameId::from_uuid(path.into_inner()),
        player_id: PlayerId::from_uuid(body.player_id),
        amount: body.amount,
    }).await?;
    state.metrics.bets_placed.add(1, &[]);
    Ok(HttpResponse::Ok().finish())
}
```

**`web::Data<T>`**: Actix-web's way to share state. Internally `Arc<T>`. `clone()` chỉ clone Arc, không clone T.

**`path.into_inner()`**: consume path extractor, lấy `Uuid` bên trong.

**`#[post("/games/{id}/bet")]`**: macro đăng ký route. Actix-web sẽ dispatch POST requests tới path pattern này vào function này.

### 7.4 WebSocket Architecture

```
Client Browser
    │ WebSocket upgrade (GET /ws/games/{id})
    ▼
ws_game() handler
    │ actix_ws::handle() → (HttpResponse, Session, MessageStream)
    │ actix_web::rt::spawn(session_loop(...))
    ▼
session_loop() [background task]
    │
    ├── tokio::select! {
    │       rx.recv()        ← nhận từ Broadcaster
    │       msg_stream.next() ← nhận từ client (ping/close)
    │   }
    │
    └── session.text(payload) → gửi JSON tới client
```

```rust
async fn session_loop(...) {
    let mut rx = broadcaster.subscribe(); // subscribe vào broadcast channel
    metrics.ws_connections.add(1, &[]);

    loop {
        tokio::select! {
            // Branch 1: có event mới từ Redis Stream
            Ok(msg) = rx.recv() => {
                if msg.game_id == game_id {  // filter: chỉ events của game này
                    session.text(msg.payload.as_ref()).await;
                }
            }
            // Branch 2: client gửi gì đó
            Some(Ok(Message::Ping(bytes))) = msg_stream.next() => {
                session.pong(&bytes).await; // heartbeat
            }
            Some(Ok(Message::Close(_))) = ... => { break; }
        }
    }
    metrics.ws_connections.add(-1, &[]);
}
```

**`tokio::select!`**: poll nhiều futures đồng thời, xử lý cái nào ready trước. Non-blocking — không chiếm thread khi không có gì.

**`broadcaster.subscribe()`**: trả về `Receiver` của `tokio::sync::broadcast`. Mỗi WS connection có receiver riêng, tất cả đều nhận cùng message.

**`RecvError::Lagged`**: nếu client quá chậm, broadcast buffer đầy, một số messages bị drop. Log warning nhưng không disconnect client.

### 7.5 Stream Reader

```rust
pub fn spawn(conn: ConnectionManager, broadcaster: Arc<Broadcaster>) {
    tokio::spawn(async move {
        run(conn, broadcaster).await; // chạy mãi mãi
    });
}

async fn run(mut conn: ConnectionManager, broadcaster: Arc<Broadcaster>) {
    let mut last_id = "$".to_string(); // "$" = chỉ đọc messages mới

    loop {
        match read_batch(&mut conn, &last_id).await {
            Ok(Some(entries)) => {
                for (id, game_id, payload) in entries {
                    broadcaster.publish(BroadcastMsg { game_id, payload: payload.into() });
                    last_id = id; // update cursor
                }
            }
            Ok(None) => {} // BLOCK timeout, không có gì, tiếp tục loop
            Err(e) => {
                tracing::error!(err = %e, "stream error");
                tokio::time::sleep(Duration::from_secs(2)).await; // backoff
            }
        }
    }
}
```

**`XREAD BLOCK 2000`**: block Redis connection tối đa 2 giây, return khi có message mới. Hiệu quả hơn polling (không busy-wait).

**`last_id = "$"`**: chỉ đọc messages kể từ khi server start. Không replay lịch sử — WebSocket clients chỉ nhận events mới. Nếu cần replay, dùng game ID làm cursor và read từ đầu.

**`payload: payload.into()`**: convert `String` → `Arc<str>`. Broadcast clone `Arc<str>` (cheap, chỉ increment ref count) thay vì clone `String` (allocate heap memory) cho mỗi receiver.

---

## 8. Observability

### 8.1 Tracing (OpenTelemetry + Jaeger)

**Ba khái niệm chính:**

```
Trace: toàn bộ journey của một request
 └── Span: một operation trong trace
      ├── Attributes: metadata (game.id, player.id, ...)
      ├── Events: log entries within span
      └── Child spans: nested operations
```

```rust
// telemetry.rs
pub fn init(service_name: &str, otlp_endpoint: &str) -> anyhow::Result<()> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()                           // gRPC transport
                .with_endpoint(otlp_endpoint)      // Jaeger OTLP receiver
        )
        .with_trace_config(
            sdktrace::config().with_resource(Resource::new(vec![
                KeyValue::new("service.name", service_name.to_string()),
            ]))
        )
        .install_batch(Tokio)  // batch exporter (không block async runtime)
        .context("...")?;

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env()...)    // RUST_LOG env
        .with(fmt::layer().json().flatten_event(true)) // JSON logs
        .with(OpenTelemetryLayer::new(tracer))         // OTel spans
        .init();
}
```

**Batch exporter vs Sync exporter:**  
Batch: tích lũy spans, gửi theo batch → ít network calls, không block async task.
Sync: gửi ngay mỗi span → latency tăng, không phù hợp production.

**`tracing-actix-web`**:  
`TracingLogger::default()` middleware tự động:
- Tạo span cho mỗi HTTP request
- Add attributes: `http.method`, `http.route`, `http.status_code`, `http.target`
- Kết nối với trace context từ request headers (W3C TraceContext)

### 8.2 Metrics (Prometheus)

```
OTel Counter.add(1)
    │ qua PrometheusExporter
    ▼
prometheus::Registry (in-memory)
    │ TextEncoder.encode()
    ▼
GET /metrics → Prometheus text format
    │ Prometheus scrapes mỗi 15s
    ▼
Grafana query → Dashboard
```

```rust
// metrics.rs
pub fn init_metrics() -> anyhow::Result<(AppMetrics, Registry)> {
    let registry = Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()?;

    let provider = SdkMeterProvider::builder()
        .with_reader(exporter)  // Prometheus là "reader" của meter provider
        .build();

    global::set_meter_provider(provider); // set global → meter() calls dùng nó

    Ok((AppMetrics::new(), registry))
}
```

**`AppMetrics::new()` gọi sau khi set provider:**  
`global::meter("blackjack")` cần provider đã được set trước. Thứ tự trong `main.rs`: init metrics → init tracer → start server.

**`Counter` vs `UpDownCounter`:**
- `Counter`: chỉ tăng (bets_placed, games_created)
- `UpDownCounter`: tăng/giảm (active_games, ws_connections)

**Metrics trong route handlers:**
```rust
// Sau khi operation thành công:
state.metrics.bets_placed.add(1, &[]);
state.metrics.chips_wagered.add(body.amount as u64, &[]);
```

`&[]` = không có attributes. Có thể thêm: `&[KeyValue::new("outcome", "win")]` để breakdown theo dimension.

### 8.3 Structured Logging

```rust
tracing::info!(
    addr = cfg.bind_addr(),
    redis = cfg.redis_url,
    "starting blackjack API"
);
```

Output JSON (do `.json()` trong subscriber):
```json
{
  "timestamp": "2025-05-08T10:00:00Z",
  "level": "INFO",
  "message": "starting blackjack API",
  "addr": "0.0.0.0:8080",
  "redis": "redis://localhost:6379",
  "span": { "name": "main" }
}
```

**Tại sao structured logging?**  
Log aggregation tools (Loki, Elasticsearch) parse JSON tốt hơn plain text. Có thể filter/query: `{addr="0.0.0.0:8080"}`.

**`flatten_event(true)`**: merge event fields vào top level JSON thay vì nest dưới `"fields"` key.

---

## 9. Chạy project

### Bước 1: Khởi động infrastructure

```bash
docker-compose up -d
```

Services:
- **Redis** `localhost:6379` — cache và message queue
- **Jaeger UI** `localhost:16686` — xem traces
- **Prometheus** `localhost:9090` — metrics storage
- **Grafana** `localhost:3000` — dashboard (admin/admin)

### Bước 2: Cấu hình

```bash
# .env (đã có sẵn)
RUST_LOG=info
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
REDIS_URL=redis://localhost:6379
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
OTEL_SERVICE_NAME=blackjack-api
```

### Bước 3: Chạy server

```bash
cargo run -p api
```

### Bước 4: Test endpoints

```bash
# Tạo player
curl -X POST http://localhost:8080/api/players \
  -H "Content-Type: application/json" \
  -d '{"name":"Alice","initial_balance":1000}'
# → {"id":"<player-uuid>","name":"Alice","balance":1000}

# Tạo game
curl -X POST http://localhost:8080/api/games
# → {"id":"<game-uuid>"}

# Join game
curl -X POST http://localhost:8080/api/games/<game-uuid>/join \
  -H "Content-Type: application/json" \
  -d '{"player_id":"<player-uuid>"}'

# Mở betting
curl -X POST http://localhost:8080/api/games/<game-uuid>/betting

# Đặt cược 100 chips
curl -X POST http://localhost:8080/api/games/<game-uuid>/bet \
  -H "Content-Type: application/json" \
  -d '{"player_id":"<player-uuid>","amount":100}'

# Deal
curl -X POST http://localhost:8080/api/games/<game-uuid>/deal

# Xem state
curl http://localhost:8080/api/games/<game-uuid>

# Hit
curl -X POST http://localhost:8080/api/games/<game-uuid>/hit \
  -H "Content-Type: application/json" \
  -d '{"player_id":"<player-uuid>"}'

# Stand
curl -X POST http://localhost:8080/api/games/<game-uuid>/stand \
  -H "Content-Type: application/json" \
  -d '{"player_id":"<player-uuid>"}'

# Dealer plays
curl -X POST http://localhost:8080/api/games/<game-uuid>/dealer

# Settle (tính kết quả)
curl -X POST http://localhost:8080/api/games/<game-uuid>/settle

# Metrics
curl http://localhost:8080/metrics
```

### Bước 5: WebSocket

```bash
# Cài wscat nếu chưa có
npm install -g wscat

# Connect
wscat -c ws://localhost:8080/ws/games/<game-uuid>

# Khi hit/stand/deal trong tab khác → nhận event JSON ở đây
```

---

## 10. Luồng game hoàn chỉnh

```
Player A                API Server                  Redis
─────────────────────── ──────────────────────────  ──────────────
POST /api/players    →  create_player()          →  SET bj:player:{id}
POST /api/games      →  create_game()            →  SET bj:game:{id}
                                                    SADD bj:games:active {id}
POST /games/{g}/join →  join_game()              →  update game JSON
POST /games/{g}/betting→ open_betting()          →  update game JSON
POST /games/{g}/bet  →  place_bet()              →  deduct chips
                                                    update game (bet recorded)

[WebSocket connects to /ws/games/{g}]
[Stream reader subscribes to bj:events]

POST /games/{g}/deal →  deal()                   →  update game (cards dealt)
                        drain_events()
                        event_pub.publish_all()  →  XADD bj:events * ...
                                                 ←  stream reader reads
                                                 →  broadcaster.publish()
                    ←← WebSocket: {"type":"card_dealt",...}  (all connected clients)

POST /games/{g}/hit  →  hit()                    →  update game
                        publish CardDealt event  →  XADD
                    ←← WebSocket: {"type":"card_dealt",...}

POST /games/{g}/stand→  stand()                  →  update game
                        publish PlayerStood      →  XADD
                    ←← WebSocket: {"type":"player_stood",...}

POST /games/{g}/dealer→ play_dealer()            →  update game
                        publish GameEnded        →  XADD
                    ←← WebSocket: {"type":"game_ended","dealer_score":19}

POST /games/{g}/settle→ settle()                 →  credit winners
                                                    SREM bj:games:active {id}
                    ←  [{"outcome":"win","return_amount":200}]
```

---

## Tóm tắt các Pattern được dùng

| Pattern | Ở đâu | Tại sao |
|---|---|---|
| **Newtype** | `GameId`, `PlayerId`, `Chips`, `Bet` | Type safety tại compile time |
| **Value Object** | `Card`, `Hand`, `Bet` | Immutable, equality by value |
| **Entity** | `Player`, `Game` | Identity, mutable state |
| **Aggregate Root** | `Game` | Single entry point, enforce invariants |
| **Domain Events** | `DomainEvent` enum | Decouple domain từ infrastructure |
| **Collect-then-Dispatch** | `pending_events` trong `Game` | Atomic: save + publish hoặc không |
| **Repository Pattern** | `GameRepository` trait | Decouple domain từ storage |
| **Ports & Adapters** | traits trong `domain/`, impls trong `infrastructure/` | Swap storage dễ dàng |
| **CQRS** | `GameCommandService` vs `GameQueryService` | Tách write/read path |
| **Factory** | `build_repos()` | Centralize dependency wiring |
| **State Machine** | `GameStatus` enum + guards | Prevent invalid state transitions |
| **DTO** | `GameStateDto`, `PlayerDto` | Decouple API từ domain model |
| **Broadcast Channel** | `tokio::sync::broadcast` | Fan-out events to multiple WS clients |
| **Structured Logging** | `tracing` macros | Machine-readable, queryable logs |
