import {
    ServerUpdate,
    ServerActionRequest,
    SpecialCardType,
    DealersChoiceResp,
    PokerViewState,
    PlayerViewState,
    TableViewState,
    CardViewState,
    PokerVariantDesc,
    PokerVariants,
    PokerVariantSelector,
    TableConfig,
    AnteRuleDesc,
    AnteRuleChangeDesc,
    ServerTableParameters,
} from "./pokerrs.ts";

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
    api<ServerUpdate>(`/gamediff?send_string_log=1&player=${player_id}&table_id=${current_table_id}&start_from=${start_from}${action_request_param}`).then(update => {
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

var clicked_cards: Array<number> | null = null;
var max_can_replace: number = 0;
function clicked_card(cidx: number) {
    const call_button = <HTMLInputElement>document.getElementById("call_button")!;
    const player_screen = document.getElementById("player_div")!;
    const player_cards = player_screen.getElementsByClassName("card_container")[0]!.children;
    if (clicked_cards) {
        let idx = clicked_cards.indexOf(cidx);
        if (idx != -1) {
            clicked_cards.splice(idx, 1);
            player_cards[cidx].classList.remove("selected");
            player_cards[cidx].classList.add("deselected");
        } else if (clicked_cards.length < max_can_replace) {
            clicked_cards.push(cidx);
            player_cards[cidx].classList.add("selected");
            player_cards[cidx].classList.remove("deselected");
        }
        call_button.value = `Replace ${clicked_cards.length} cards`;
    }
}

function make_card(card: CardViewState): HTMLElement {
    let img = document.createElement("img");
    switch(card.kind) {
        case "Visible": {
            let suit = ["S", "H", "D", "C"][card.data.card.suit];
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
            img.src = `card_images/155px-Minicard_${rank}${suit}.svg.png`;
            img.alt = `${rank} of ${suit}s`;
            break;
        }
        case "Invisible": {
            img.src = `card_images/155px-Minicard_back.svg.png`;
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
    const player_screen = document.getElementById("player_div")!;
    const player_label = player_screen.getElementsByClassName("player_name")[0]!;
    const player_chips_label = player_screen.getElementsByClassName("player_chips")[0]!;
    const player = viewstate?.players[viewstate.role];
    const player_cards = player_screen.getElementsByClassName("card_container")[0]!;
    player_label.innerHTML = player_id;
    if (player) {
        const player_chips = player.chips - player.total_bet - (viewstate?.bet_this_round[viewstate.role] ?? 0);
        player_chips_label.innerHTML = `${player_chips} 🪙`;
        player_cards.innerHTML = "";
        for (let cidx = 0; cidx < player.hand.length; cidx += 1) {
            const card = player.hand[cidx];
            let card_view = make_card(card);
            const fixed_cidx = cidx;
            if (clicked_cards?.includes(cidx)) {
                card_view.classList.add("selected");
            } else {
                card_view.classList.add("deselected");
            }
            card_view.addEventListener('click', () => {
                clicked_card(fixed_cidx);
            });
            player_cards.appendChild(card_view);
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
    const replace_cards_label = <HTMLElement>document.getElementById("replace_cards_label")!;
    const dealers_choice_modal = <HTMLElement>document.getElementById("dealers_choice_modal")!;
    const dealers_choice_list = <HTMLElement>document.getElementById("dealers_choice_list")!;

    call_button.value = "Call";
    dealers_choice_list.innerHTML = "";
    dealers_choice_modal.classList.add("hidden");

    if (action && viewstate && action.kind == "Bet") {
        replace_cards_label.classList.add("hidden");

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
    } else if (action && viewstate && action.kind == "Replace") {
        fold_button.setAttribute("disabled", "");
        bet_button.setAttribute("disabled", "");
        bet_input.setAttribute("disabled", "");
        bet_input.value = "";

        call_button.removeAttribute("disabled");
        call_button.value = `Replace 0 cards`;

        replace_cards_label.classList.remove("hidden");
        replace_cards_label.innerHTML = `Replace up to ${action.data.max_can_replace} cards. Click cards to select/deselect`;
        max_can_replace = action.data.max_can_replace;

        clicked_cards = [];
    } else if (action && action.kind == "DealersChoice") {
        call_button.setAttribute("disabled", "");
        fold_button.setAttribute("disabled", "");
        bet_button.setAttribute("disabled", "");
        bet_input.setAttribute("disabled", "");
        replace_cards_label.classList.add("hidden");
        bet_input.value = "";
        dealers_choice_list.innerHTML = "";

        dealers_choice_modal.classList.remove("hidden");

        for (let vidx = 0; vidx < action.data.variants.length; vidx += 1) {
            const desc = action.data.variants[vidx];
            const button = document.createElement("input");
            button.setAttribute("type", "button");
            button.classList.add("dealers_choice_button");
            button.value = desc.name;
            button.addEventListener('click', () => {
                dealers_choice(vidx);
            });
            dealers_choice_list.appendChild(button);
        }
    } else {
        call_button.setAttribute("disabled", "");
        fold_button.setAttribute("disabled", "");
        bet_button.setAttribute("disabled", "");
        bet_input.setAttribute("disabled", "");
        replace_cards_label.classList.add("hidden");
        bet_input.value = "";
    }
}

var client_logs: Array<Array<String>> = [];
var log_page: number = 0;
var current_table_id = "";

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
    const variant_label = document.getElementById("variant_name_label")!;
    game_screen?.classList.remove("hidden");
    const viewstate = update.player?.viewstate ?? null;
    const action = update.player?.action_requested ?? null;
    draw_players(player_id, viewstate, update.table);
    draw_action(action, viewstate);

    variant_label.innerHTML = update.table.running_variant?.name ?? "Waiting for next game...";

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
    } else {
        pot_label.innerHTML = '';
        community_cards.innerHTML = "";
    }

    update_logs(update, null);
}

function join() {
    const player_input = document.getElementById("name_input");
    const player_id = (<HTMLInputElement>player_input).value.trim();

    if (player_id.length > 0) {
        const name_screen = document.getElementById("name_screen");
        name_screen?.classList.add("hidden");
        api<ServerUpdate>(`/game?player=${player_id}&table_id=${current_table_id}`).then(update => {
            draw(player_id, update);
            const action = update.player?.action_requested ?? null;
            fetch_update(player_id, 0, action);
        }).catch(err => {
            console.log(`err: ${err}`);
        });
    }
}

function reset_input() {
    draw_action(null, null);
}

function dealers_choice(idx: number) {
    const player_input = document.getElementById("name_input");
    const player_id = (<HTMLInputElement>player_input).value.trim();
    const two_wild = (<HTMLInputElement>document.getElementById("two_wild")).checked;
    const king_axe = (<HTMLInputElement>document.getElementById("king_axe")).checked;
    const dealers_choice_list = <HTMLElement>document.getElementById("dealers_choice_list");
    let revert = () => {
        for (const element of dealers_choice_list.getElementsByClassName("dealers_choice_button")) {
            element.removeAttribute("disabled");
        }
    };
    for (const element of dealers_choice_list.getElementsByClassName("dealers_choice_button")) {
        (<HTMLElement>element).setAttribute("disabled", "");
    }
    let special_cards = [];
    if (two_wild) {
        for (let suit=0; suit<4; suit+=1) {
            special_cards.push({
                wtype: <SpecialCardType>"Wild",
                card: {rank: 1, suit: suit}
            });
        }
    }
    if (king_axe) {
        special_cards.push({
            wtype: <SpecialCardType>"WinsItAll",
            card: {rank: 12, suit: 2}
        });
    }
    let resp: DealersChoiceResp = {
        variant_idx: idx,
        special_cards: special_cards,
    };
    fetch(`/dealers_choice?player=${player_id}&table_id=${current_table_id}`, {
        method: "POST",
        body: JSON.stringify(resp)
    }).then(resp => {
        if (resp.ok) {
            reset_input();
        } else {
            revert();
        }
    }).catch(err => {
        revert();
    });
}

function replace() {
    const player_input = document.getElementById("name_input");
    const player_id = (<HTMLInputElement>player_input).value.trim();
    const call_button = document.getElementById("call_button")!;
    if (clicked_cards) {
        let replace_resp = clicked_cards;
        clicked_cards = null;
        call_button.setAttribute("disabled", "");
        let revert = () => {
            clicked_cards = [];
            call_button.removeAttribute("disabled");
            for (const cidx of replace_resp) {
                clicked_card(cidx);
            }
        };
        fetch(`/replace?player=${player_id}&table_id=${current_table_id}`, {
            method: "POST",
            body: JSON.stringify(replace_resp)
        }).then(resp => {
            if (resp.ok) {
                reset_input();
            } else {
                revert();
            }
        }).catch(err => {
            revert();
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
    fetch(`/bet?player=${player_id}&table_id=${current_table_id}`, {
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
    fetch(`/start?table_id=${current_table_id}`, {
        method: 'POST'
    });
}

function add_bot(skill: number) {
    fetch(`/add_bot?table_id=${current_table_id}&bot_skill=${skill}`, {
        method: 'POST'
    });
}

function call_button_clicked() {
    if (clicked_cards) {
        replace();
    } else {
        bet("call");
    }
}

function change_variant_clicked(change: 'add' | 'remove') {
    const included_list = document.getElementById("variants_included")!;
    const excluded_list = document.getElementById("variants_excluded")!;

    if (change == 'add') {
        let new_included = [];
        for (const opt of included_list.children) {
            new_included.push(opt);
        }
        let new_excluded = [];
        for (const opt of excluded_list.children) {
            if ((<HTMLOptionElement>opt).selected) {
                new_included.push(opt);
            } else {
                new_excluded.push(opt);
            }
        }
        included_list.replaceChildren(...new_included);
        excluded_list.replaceChildren(...new_excluded);
    } else if (change == 'remove') {
        let new_included = [];
        let new_excluded = [];
        for (const opt of excluded_list.children) {
            new_excluded.push(opt);
        }
        for (const opt of included_list.children) {
            if ((<HTMLOptionElement>opt).selected) {
                new_excluded.push(opt);
            } else {
                new_included.push(opt);
            }
        }
        included_list.replaceChildren(...new_included);
        excluded_list.replaceChildren(...new_excluded);
    }
}

function read_ante_rule(): AnteRuleDesc {
    const min_bet_input = <HTMLInputElement>document.getElementById("min_bet_input")!;
    const bet_increase_time = <HTMLInputElement>document.getElementById("bet_increase_time")!;
    const double_bet_input = <HTMLInputElement>document.getElementById("double_bet_input")!;
    const rounds_option = <HTMLInputElement>document.getElementById("bet_increase_rounds_option")!;
    const minutes_option = <HTMLInputElement>document.getElementById("bet_increase_minutes_option")!;
    const blinds_radio = <HTMLInputElement>document.getElementById("ante_blinds")!;

    let change: AnteRuleChangeDesc = double_bet_input.checked ? (rounds_option.checked ? {
        kind: "MulEveryNRounds",
        data: {
            mul: 2,
            rounds: Number.parseInt(bet_increase_time.value)
        }
    } : {
        kind: "MulEveryNSeconds",
        data: {
            mul: 2,
            seconds: Number.parseInt(bet_increase_time.value) * 60
        }
    }) : {kind: "Constant"};

    let retval: AnteRuleDesc = {
        starting_value: Number.parseInt(min_bet_input.value),
        blinds: blinds_radio.checked,
        change: change,
    };

    return retval;
}

function create_table() {
    const create_table_button = document.getElementById("create_table_button")!;
    const table_settings_modal = document.getElementById("table_settings_modal")!;
    const max_players_input = <HTMLInputElement>document.getElementById("max_players_input")!;
    const starting_chips_input = <HTMLInputElement>document.getElementById("starting_chips_input")!;
    const dealers_choice_input = <HTMLInputElement>document.getElementById("dealers_choice_input")!;
    const rotation_input = <HTMLInputElement>document.getElementById("rotation_input")!;
    const included_list = document.getElementById("variants_included")!;

    create_table_button.setAttribute("disabled", "");

    let variants: Array<PokerVariantDesc> = [];
    for (const opt of included_list.children) {
        variants.push({name: (<HTMLOptionElement>opt).value});
    }

    let selector: PokerVariantSelector = {
        kind: rotation_input.checked ? "Rotation" : "DealersChoice",
        data: {
            descs: variants,
        },
    };

    let config: TableConfig = {
        max_players: Number.parseInt(max_players_input.value),
        starting_chips: Number.parseInt(starting_chips_input.value),
        variant_selector: selector,
    };

    console.log(config);

    let params: ServerTableParameters = {
        table_config: config,
        ante_rule: read_ante_rule(),
    };

    fetch('/create_table', {
        method: "POST",
        body: JSON.stringify(params)
    }).then(resp => {
        create_table_button.removeAttribute("disabled");
        if (resp.ok) {
            table_settings_modal.classList.add("hidden");
            resp.json().then(table_id => {
                window.location.href = `table?table_id=${table_id}`;
            });
        } else {
        }
    }).catch(err => {
        create_table_button.removeAttribute("disabled");
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
    const add_easy_bot_button = document.getElementById("add_easy_bot_button")!;
    const add_medium_bot_button = document.getElementById("add_medium_bot_button")!;
    const add_variant_button = document.getElementById("settings_add_variant_button")!;
    const remove_variant_button = document.getElementById("settings_remove_variant_button")!;
    const table_settings_modal = document.getElementById("table_settings_modal")!;
    const create_table_button = document.getElementById("create_table_button")!;

    player_input.addEventListener('click', () => {
        join();
    });
    call_button.addEventListener('click', () => {
        call_button_clicked();
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
    add_easy_bot_button.addEventListener('click', () => {
        add_bot(1);
    });
    add_medium_bot_button.addEventListener('click', () => {
        add_bot(2);
    });
    add_variant_button.addEventListener('click', () => {
        change_variant_clicked('add');
    });
    remove_variant_button.addEventListener('click', () => {
        change_variant_clicked('remove');
    });
    create_table_button.addEventListener('click', () => {
        create_table();
    });

    const current_url = new URL(window.location.href);
    const table_id = current_url.searchParams.get("table_id");
    if (table_id) {
        table_settings_modal.classList.add("hidden");
        current_table_id = table_id;
    }
});
