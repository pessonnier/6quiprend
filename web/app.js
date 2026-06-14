const boardElement = document.querySelector("#board");
const boardCountElement = document.querySelector("#board-count");
const connectionStatusElement = document.querySelector("#connection-status");
const myHandElement = document.querySelector("#my-hand");
const opponentsElement = document.querySelector("#opponents");
const playerNameForm = document.querySelector("#player-name-form");
const playerNameInput = document.querySelector("#player-name");
const CARD_WIDTH = 86;
const CARD_HEIGHT = 122;

let gameState = { width: 960, height: 620, me: null, players: [], boardCards: [] };
let draggedCard = null;
let pendingGameState = null;
let lastSentAt = 0;

bootstrap();

playerNameForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const name = playerNameInput.value.trim();

  if (!name) {
    return;
  }

  await fetch("/api/session/name", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
});

async function bootstrap() {
  try {
    const response = await fetch("/api/session");
    gameState = await response.json();
    renderGame(gameState);
    connectToBoardEvents();
  } catch (error) {
    setConnectionStatus("Hors ligne");
  }
}

function connectToBoardEvents() {
  const events = new EventSource("/api/events");

  events.addEventListener("open", () => setConnectionStatus("Synchronise"));
  events.addEventListener("error", () => setConnectionStatus("Reconnexion..."));
  events.addEventListener("board", (event) => {
    gameState = JSON.parse(event.data);

    if (draggedCard) {
      pendingGameState = gameState;
      renderGame(gameState, draggedCard.id);
      return;
    }

    renderGame(gameState);
  });
}

function renderGame(nextGameState, skippedCardId = null) {
  boardElement.style.setProperty("--board-width", nextGameState.width);
  boardElement.style.setProperty("--board-height", nextGameState.height);
  boardCountElement.textContent = pluralize(nextGameState.boardCards.length, "carte");

  if (nextGameState.me && document.activeElement !== playerNameInput) {
    playerNameInput.value = nextGameState.me.name;
  }

  renderBoard(nextGameState.boardCards, skippedCardId);
  renderOwnHand(nextGameState.me?.hand ?? [], skippedCardId);
  renderOpponents(nextGameState.players.filter((player) => !player.isCurrentPlayer));
}

function renderBoard(boardCards, skippedCardId = null) {
  const activeCardIds = new Set(boardCards.map((card) => card.id));

  for (const card of boardCards) {
    let cardElement = boardElement.querySelector(`[data-card-id="${card.id}"]`);

    if (!cardElement) {
      cardElement = createCardElement(card, "board");
      boardElement.append(cardElement);
    }

    updateVisibleCard(cardElement, card);

    if (card.id !== skippedCardId) {
      moveBoardCardElement(cardElement, card.x, card.y);
    }
  }

  for (const cardElement of boardElement.querySelectorAll(".card")) {
    if (!activeCardIds.has(cardElement.dataset.cardId)) {
      cardElement.remove();
    }
  }
}

function renderOwnHand(handCards, skippedCardId = null) {
  const activeCardIds = new Set(handCards.map((card) => card.id));

  for (const card of handCards) {
    let cardElement = myHandElement.querySelector(`[data-card-id="${card.id}"]`);

    if (!cardElement) {
      cardElement = createCardElement(card, "hand");
      myHandElement.append(cardElement);
    }

    updateVisibleCard(cardElement, card);
    cardElement.style.transform = card.id === skippedCardId ? cardElement.style.transform : "";
  }

  for (const cardElement of myHandElement.querySelectorAll(".card")) {
    if (!activeCardIds.has(cardElement.dataset.cardId)) {
      cardElement.remove();
    }
  }
}

function renderOpponents(opponents) {
  opponentsElement.replaceChildren();

  if (opponents.length === 0) {
    const emptyElement = document.createElement("p");
    emptyElement.className = "empty-opponents";
    emptyElement.textContent = "Aucun autre joueur";
    opponentsElement.append(emptyElement);
    return;
  }

  for (const opponent of opponents) {
    const opponentElement = document.createElement("section");
    opponentElement.className = "opponent";
    opponentElement.innerHTML = `
      <div class="opponent-header">
        <strong></strong>
        <span>${pluralize(opponent.handCount, "carte")}</span>
      </div>
      <div class="hidden-hand" aria-label="Main cachee de ${escapeHtml(opponent.name)}"></div>
    `;
    opponentElement.querySelector("strong").textContent = opponent.name;

    const hiddenHandElement = opponentElement.querySelector(".hidden-hand");
    for (let index = 0; index < opponent.handCount; index += 1) {
      hiddenHandElement.append(createCardBackElement());
    }

    opponentsElement.append(opponentElement);
  }
}

function createCardElement(card, zone) {
  const cardElement = document.createElement("article");
  cardElement.className = "card";
  cardElement.dataset.cardId = card.id;
  cardElement.dataset.zone = zone;
  cardElement.setAttribute("aria-label", `Carte ${card.label}`);
  cardElement.innerHTML = `
    <span class="card-corner"></span>
    <strong class="card-number"></strong>
    <span class="card-mark" aria-hidden="true">V</span>
    <span class="card-corner card-corner-bottom"></span>
  `;

  cardElement.addEventListener("pointerdown", (event) => startDragging(event, cardElement));
  return cardElement;
}

function createCardBackElement() {
  const cardBack = document.createElement("span");
  cardBack.className = "card-back";
  cardBack.setAttribute("aria-hidden", "true");
  return cardBack;
}

function updateVisibleCard(cardElement, card) {
  cardElement.dataset.zone = cardElement.parentElement === boardElement ? "board" : "hand";
  cardElement.querySelectorAll(".card-corner").forEach((corner) => {
    corner.textContent = card.label;
  });
  cardElement.querySelector(".card-number").textContent = card.label;
  cardElement.style.setProperty("--card-color", card.color);
  cardElement.setAttribute("aria-label", `Carte ${card.label}`);
}

function startDragging(event, cardElement) {
  if (event.button !== 0) {
    return;
  }

  const cardRect = cardElement.getBoundingClientRect();
  draggedCard = {
    id: cardElement.dataset.cardId,
    originZone: cardElement.parentElement === boardElement ? "board" : "hand",
    offsetX: event.clientX - cardRect.left,
    offsetY: event.clientY - cardRect.top,
  };

  cardElement.classList.add("is-dragging");
  cardElement.setPointerCapture(event.pointerId);
  cardElement.addEventListener("pointermove", dragCard);
  cardElement.addEventListener("pointerup", stopDragging, { once: true });
  cardElement.addEventListener("pointercancel", stopDragging, { once: true });

  updateDraggedCardPosition(event, cardElement, true);
}

function dragCard(event) {
  if (!draggedCard) {
    return;
  }

  updateDraggedCardPosition(event, event.currentTarget);
}

async function stopDragging(event) {
  const cardElement = event.currentTarget;
  cardElement.classList.remove("is-dragging");
  cardElement.removeEventListener("pointermove", dragCard);

  if (draggedCard) {
    await finishDrag(event, cardElement);
  }

  draggedCard = null;

  if (pendingGameState) {
    renderGame(pendingGameState);
    pendingGameState = null;
  }
}

function updateDraggedCardPosition(event, cardElement, forceSend = false) {
  if (draggedCard.originZone === "board") {
    const boardRect = boardElement.getBoundingClientRect();

    if (isPointInside(event.clientX, event.clientY, boardRect)) {
      const boardPoint = eventToBoardPoint(event, boardRect);
      moveBoardCardElement(cardElement, boardPoint.x, boardPoint.y);
      queueBoardMove(cardElement.dataset.cardId, boardPoint.x, boardPoint.y, forceSend);
    } else {
      moveBoardCardFreely(cardElement, event, boardRect);
    }

    return;
  }

  const handRect = myHandElement.getBoundingClientRect();
  cardElement.style.transform = `translate(${event.clientX - handRect.left - draggedCard.offsetX}px, ${event.clientY - handRect.top - draggedCard.offsetY}px)`;
}

async function finishDrag(event, cardElement) {
  const boardRect = boardElement.getBoundingClientRect();
  const handRect = myHandElement.getBoundingClientRect();
  const cardId = cardElement.dataset.cardId;

  if (draggedCard.originZone === "hand" && isPointInside(event.clientX, event.clientY, boardRect)) {
    const boardPoint = eventToBoardPoint(event, boardRect);
    await playCard(cardId, boardPoint.x, boardPoint.y);
    return;
  }

  if (draggedCard.originZone === "board" && isPointInside(event.clientX, event.clientY, handRect)) {
    await takeCard(cardId);
    return;
  }

  if (draggedCard.originZone === "board") {
    const boardPoint = eventToBoardPoint(event, boardRect);
    await moveBoardCard(cardId, boardPoint.x, boardPoint.y);
  }
}

function eventToBoardPoint(event, boardRect) {
  const scaleX = gameState.width / boardRect.width;
  const scaleY = gameState.height / boardRect.height;
  const x = clamp((event.clientX - boardRect.left - draggedCard.offsetX) * scaleX, 0, gameState.width - CARD_WIDTH);
  const y = clamp((event.clientY - boardRect.top - draggedCard.offsetY) * scaleY, 0, gameState.height - CARD_HEIGHT);

  return { x: Math.round(x), y: Math.round(y) };
}

function moveBoardCardElement(cardElement, x, y) {
  const scale = boardElement.getBoundingClientRect().width / gameState.width;
  cardElement.style.transform = `translate(${x * scale}px, ${y * scale}px) scale(${scale})`;
}

function moveBoardCardFreely(cardElement, event, boardRect) {
  const scale = boardElement.getBoundingClientRect().width / gameState.width;
  const x = event.clientX - boardRect.left - draggedCard.offsetX * scale;
  const y = event.clientY - boardRect.top - draggedCard.offsetY * scale;
  cardElement.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
}

function queueBoardMove(cardId, x, y, forceSend = false) {
  const now = Date.now();

  if (!forceSend && now - lastSentAt < 90) {
    return;
  }

  lastSentAt = now;
  moveBoardCard(cardId, x, y);
}

async function moveBoardCard(cardId, x, y) {
  await sendCardAction(cardId, "move", { x, y });
}

async function playCard(cardId, x, y) {
  await sendCardAction(cardId, "play", { x, y });
}

async function takeCard(cardId) {
  await sendCardAction(cardId, "take", {});
}

async function sendCardAction(cardId, action, payload) {
  try {
    const response = await fetch(`/api/cards/${encodeURIComponent(cardId)}/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      setConnectionStatus("Action refusee");
    }
  } catch (error) {
    setConnectionStatus("Hors ligne");
  }
}

function setConnectionStatus(label) {
  connectionStatusElement.textContent = label;
}

function isPointInside(x, y, rect) {
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function pluralize(count, singular) {
  return `${count} ${singular}${count > 1 ? "s" : ""}`;
}

function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
