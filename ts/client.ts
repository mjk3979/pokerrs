import {ServerUpdate, ServerActionRequest, PokerViewState, PlayerViewState, TableViewState, CardViewState} from "./pokerrs.ts";

function api<T>(url: string): Promise<T> {
  return fetch(url)
    .then(response => {
      if (!response.ok) {
        throw new Error(response.statusText)
      }
      return response.json()
    })

}

function fetch_update(player_id: string, start_from: number, known_action_requested: ServerActionRequest | null) {
    let action_request_param = `&known_action_requested=${encodeURI(JSON.stringify(known_action_requested))}`;
    api<ServerUpdate>(`/gamediff?send_string_log=1&player=${player_id}&start_from=${start_from}${action_request_param}`).then(update => {
        console.log(update);
        for (let log_update of update.log) {
            start_from += log_update.log.length;
        }
        let action_requested = update.player?.action_requested ?? null;
        draw(player_id, update);
        console.log(`fetching ${start_from} ${action_requested?.kind}`);
        fetch_update(player_id, start_from, action_requested);
    }).catch(err => {
        console.log(`error: ${err}`);
    });
}

function make_card(card: CardViewState): HTMLElement {
    let img = document.createElement("img");
    switch(card.kind) {
        case "Visible": {
            let suit = ["Spade", "Heart", "Diamond", "Club"][card.data.card.suit];
            let rank = (() => {
                const nrank = card.data.card.rank;
                if (nrank == 0) {
                    return "A";
                } else if (nrank <= 9) {
                    return `${nrank+1}`
                } else {
                    return ["J", "Q", "K"][nrank-10];
                }
            })();
            img.src = `card_images/200px-Cards-${rank}-${suit}.svg.png`;
            img.alt = `${rank} of ${suit}s`;
            break;
        }
        case "Invisible": {
            img.src = `card_images/200px-card-back.png`;
            img.alt = "Back of card";
            break;
        }
    }
    return img;
}

function make_player_screen(role: number, player_id: string, viewstate: PokerViewState): HTMLElement {
    const player = viewstate.players[role];
    const template = document.getElementById("player_div")!;
    let player_screen = <HTMLElement>template.cloneNode(true);
    player_screen.id = `player_div_${player_id}`;
    const player_label = player_screen.getElementsByClassName("player_name")[0]!;
    const player_chips_label = player_screen?.getElementsByClassName("player_chips")[0]!;
    const player_cards = player_screen?.getElementsByClassName("card_container")[0]!;
    const player_chips = player.chips - player.total_bet - (viewstate.bet_this_round[role] ?? 0);
    player_label.innerHTML = player_id;
    player_chips_label.innerHTML = `${player_chips} 🪙`;
    player_cards.innerHTML = "";
    for (const card of player.hand) {
        player_cards.appendChild(make_card(card));
    }
    return player_screen;
}

function make_table_player_screen(idx: number, player_id: string): HTMLElement {
    const template = document.getElementById("player_div")!;
    let player_screen = <HTMLElement>template.cloneNode(true);
    player_screen.id = `player_div_table_${idx}`;
    const player_label = player_screen.getElementsByClassName("player_name")[0]!;
    const player_chips_label = player_screen?.getElementsByClassName("player_chips")[0]!;
    const player_cards = player_screen?.getElementsByClassName("card_container")[0]!;
    player_label.innerHTML = player_id;
    player_chips_label.innerHTML = "";
    player_cards.innerHTML = "";
    return player_screen;
}

function make_empty_player_screen(idx: number): HTMLElement {
    const template = document.getElementById("player_div")!;
    let player_screen = <HTMLElement>template.cloneNode(true);
    player_screen.id = `player_div_empty_${idx}`;
    const player_label = player_screen.getElementsByClassName("player_name")[0]!;
    const player_chips_label = player_screen?.getElementsByClassName("player_chips")[0]!;
    const player_cards = player_screen?.getElementsByClassName("card_container")[0]!;
    player_label.innerHTML = "Empty Seat";
    player_chips_label.innerHTML = "";
    player_cards.innerHTML = "";
    return player_screen;
}

function mod(a: number, m: number): number {
    return ((a % m) + m) % m;
}

function draw_players(player_id: string, viewstate: PokerViewState | null, table: TableViewState) {
    const player_screen = document.getElementById("player_div");
    const player_label = player_screen?.getElementsByClassName("player_name")[0]!;
    const player_chips_label = player_screen?.getElementsByClassName("player_chips")[0]!;
    const player = viewstate?.players[viewstate.role];
    const player_cards = player_screen?.getElementsByClassName("card_container")[0]!;
    player_label.innerHTML = player_id;
    if (player) {
        const player_chips = player.chips - player.total_bet - (viewstate?.bet_this_round[viewstate.role] ?? 0);
        player_chips_label.innerHTML = `${player_chips} 🪙`;
        player_cards.innerHTML = "";
        for (const card of player.hand) {
            player_cards.appendChild(make_card(card));
        }
    } else {
        player_chips_label.innerHTML = "";
        player_cards.innerHTML = "";
    }

    const plist_top = document.getElementById("player_side_top")!;
    const plist_left = document.getElementById("player_side_left")!;
    const plist_right = document.getElementById("player_side_right")!;
    plist_top.innerHTML = "";
    plist_left.innerHTML = "";
    plist_right.innerHTML = "";
    const max_seats = table.config.max_players;
    let seat_map: Record<number, string> = {};
    for (const [p, s] of Object.entries(table.seats)) {
        seat_map[Number(s)] = p;
    }
    let role_map: Record<string, number> = {};
    if (table.roles) {
        for (const [r, p] of Object.entries(table.roles)) {
            role_map[p] = Number(r);
        }
    }
    let player_seat = table.seats[player_id];
    // assume player is 0 (without loss of generality)
    // start at 1
    for (let iseat=1; iseat < max_seats; ++iseat) {
        let seat = mod(iseat + player_seat, max_seats);
        const player_id = seat_map[seat];
        let mscreen = (() => {
            if (player_id && viewstate) {
                const role = role_map[player_id];
                if (role != undefined) {
                    const player = viewstate.players[role];
                    if (role == viewstate.role) {
                        return {kind: "skip"};
                    }
                    return {kind: "draw", data: make_player_screen(role, player_id, viewstate)};
                } else {
                    return {kind: "skip"};
                }
            } else if (player_id) {
                return {kind: "draw", data: make_table_player_screen(iseat, player_id)};
            } else {
                return {kind: "draw", data: make_empty_player_screen(iseat)};
            }
        })();
        if (mscreen.kind == "draw") {
            const screen = mscreen.data!;
            if ((iseat) <= (max_seats-1)/3) {
                plist_left.appendChild(screen);
            } else if ((iseat) <= (max_seats-1)*2/3) {
                plist_top.appendChild(screen);
            } else {
                plist_right.appendChild(screen);
            }
        }
    }
}

function draw_action(action: ServerActionRequest | null, viewstate: PokerViewState | null) {
    const call_button = <HTMLInputElement>document.getElementById("call_button")!;
    const fold_button = document.getElementById("fold_button")!;
    const bet_button = document.getElementById("bet_button")!;
    const bet_input = <HTMLInputElement>document.getElementById("bet_input")!;
    const call_amount_input = <HTMLInputElement>document.getElementById("call_amount_input")!;
    const bet_this_round_input = <HTMLInputElement>document.getElementById("bet_this_round_input")!;
    if (action && viewstate && action.kind == "Bet") {
        call_button.removeAttribute("disabled");
        fold_button.removeAttribute("disabled");
        bet_button.removeAttribute("disabled");
        bet_input.removeAttribute("disabled");

        const player = viewstate.players[viewstate.role];
        const bet_this_round = viewstate.bet_this_round[viewstate.role] ?? 0;
        const bettable = player.chips - player.total_bet - bet_this_round;
        const call_amount = action.data.call_amount;
        if (call_amount > 0) {
            call_button.value = `Call ${call_amount}`;
        } else {
            call_button.value = "Check";
        }
        let min = action.data.min_bet - bet_this_round;
        if (bettable < min) {
            min = bettable;
        }
        bet_input.setAttribute("min", min.toString());
        bet_input.setAttribute("max", bettable.toString());
        bet_input.value = min.toString();
        call_amount_input.value = call_amount.toString();
        bet_this_round_input.value = bet_this_round.toString();
    } else {
        call_button.setAttribute("disabled", "");
        fold_button.setAttribute("disabled", "");
        bet_button.setAttribute("disabled", "");
        bet_input.setAttribute("disabled", "");
    }
}

var client_logs: Array<Array<String>> = [];
var log_page: number = 0;

function update_logs(update: ServerUpdate | null, log_page_change: number | null) {
    const log_list = document.getElementById("log_list")!;
    const log_round_label = document.getElementById("log_round_label")!;
    log_list.innerHTML = "";
    log_round_label.innerHTML = "";
    const on_last_page = (client_logs.length == 0) || (log_page == (client_logs.length - 1));
    if (update?.slog) {
        for (let idx = 0; idx < update.slog.length; idx += 1) {
            const round_log = update.log[idx];
            const string_logs = update.slog[idx];
            while (client_logs.length <= round_log.round) {
                client_logs.push([]);
            }
            let client_log = client_logs[round_log.round];
            for (const log of string_logs) {
                client_log.push(log);
            }
        }
    }
    console.log(`${log_page}: ${client_logs}`);
    if (client_logs.length == 0) {
        return;
    }
    if (on_last_page) {
        log_page = client_logs.length - 1;
    }
    log_page += log_page_change ?? 0;
    if (log_page < 0) {
        log_page = 0;
    }
    if (log_page >= client_logs.length) {
        log_page = client_logs.length - 1;
    }
    log_round_label.innerHTML = `Round ${log_page+1}`;
    for (const log of client_logs[log_page]) {
        const log_label = document.createElement("span");
        log_label.classList.add("log_entry");
        log_label.innerHTML = log.toString();
        log_list.appendChild(log_label);
    }
}

function draw(player_id: string, update: ServerUpdate) {
    const game_screen = document.getElementById("game_screen");
    const start_server_button = document.getElementById("start_server_button")!;
    game_screen?.classList.remove("hidden");
    const viewstate = update.player?.viewstate ?? null;
    const action = update.player?.action_requested ?? null;
    draw_players(player_id, viewstate, update.table);
    draw_action(action, viewstate);

    if (update.table.running) {
        start_server_button.classList.add("hidden");
    } else {
        start_server_button.classList.remove("hidden");
        if (Object.keys(update.table.seats).length > 1) {
            start_server_button.removeAttribute("disabled");
        } else {
            start_server_button.setAttribute("disabled", "");
        }
    }

    const pot_label = document.getElementById("pot_label")!;
    const community_cards = document.getElementById("community_cards")!;
    if (viewstate) {
        let pot = 0;
        for (const [_, player] of Object.entries(viewstate.players)) {
            pot += Number(player.total_bet);
        }
        for (const [_, bet] of Object.entries(viewstate.bet_this_round)) {
            pot += Number(bet);
        }
        pot_label.innerHTML = `${pot} 🪙`;

        community_cards.innerHTML = "";
        for (const card of viewstate.community_cards) {
            community_cards.appendChild(make_card(card));
        }
    }

    update_logs(update, null);
}

function join() {
    const player_input = document.getElementById("name_input");
    const player_id = (<HTMLInputElement>player_input).value.trim();

    if (player_id.length > 0) {
        const name_screen = document.getElementById("name_screen");
        name_screen?.classList.add("hidden");
        api<ServerUpdate>(`/game?player=${player_id}`).then(update => {
            draw(player_id, update);
            const action = update.player?.action_requested ?? null;
            fetch_update(player_id, 0, action);
        }).catch(err => {
            console.log(`err: ${err}`);
        });
    }
}

function bet(action: "bet" | "fold" | "call") {
    const player_input = document.getElementById("name_input")!;
    const player_id = (<HTMLInputElement>player_input).value.trim();
    const call_button = document.getElementById("call_button")!;
    const fold_button = document.getElementById("fold_button")!;
    const bet_button = document.getElementById("bet_button")!;
    const bet_input = <HTMLInputElement>document.getElementById("bet_input")!;
    const call_amount_input = <HTMLInputElement>document.getElementById("call_amount_input")!;
    const bet_this_round_input = <HTMLInputElement>document.getElementById("bet_this_round_input")!;

    const bet_resp = (() => {
        switch (action) {
            case "bet": {
                const bet = Number.parseInt(bet_input.value) + Number.parseInt(bet_this_round_input.value);
                return {kind: "Bet", data: bet};
            }
            case "fold": {
                return {kind: "Fold"};
            }
            case "call": {
                const call_amount = Number.parseInt(call_amount_input.value);
                return {kind: "Bet", data: call_amount};
            }
        }
    })();

    call_button.setAttribute("disabled", "");
    fold_button.setAttribute("disabled", "");
    bet_button.setAttribute("disabled", "");
    bet_input.setAttribute("disabled", "");
    fetch(`/bet?player=${player_id}`, {
        method: "POST",
        body: JSON.stringify(bet_resp)
    }).then(resp => {
        if (!resp.ok) {
            call_button.removeAttribute("disabled");
            fold_button.removeAttribute("disabled");
            bet_button.removeAttribute("disabled");
            bet_input.removeAttribute("disabled");
        }
    }).catch(err => {
        call_button.removeAttribute("disabled");
        fold_button.removeAttribute("disabled");
        bet_button.removeAttribute("disabled");
        bet_input.removeAttribute("disabled");
    });
}

function start_server() {
    fetch('/start', {
        method: 'POST'
    });
}

document.addEventListener('DOMContentLoaded', () => {
    const player_input = document.getElementById("name_submit")!;
    const call_button = document.getElementById("call_button")!;
    const fold_button = document.getElementById("fold_button")!;
    const bet_button = document.getElementById("bet_button")!;
    const bet_input = document.getElementById("bet_input")!;
    const prev_round_button = document.getElementById("prev_round_button")!;
    const next_round_button = document.getElementById("next_round_button")!;
    const start_server_button = document.getElementById("start_server_button")!;
    player_input.addEventListener('click', () => {
        join();
    });
    call_button.addEventListener('click', () => {
        bet("call");
    });
    fold_button.addEventListener('click', () => {
        bet("fold");
    });
    bet_button.addEventListener('click', () => {
        bet("bet");
    });
    prev_round_button.addEventListener('click', () => {
        update_logs(null, -1);
    });
    next_round_button.addEventListener('click', () => {
        update_logs(null, 1);
    });
    start_server_button.addEventListener('click', () => {
        start_server();
    });
});
