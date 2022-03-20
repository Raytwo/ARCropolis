const MOD_MENU = "modMenu";
const SUB_MENU = "subMenu";
const categories = [
    "All",
    "Fighter",
    "Stage",
    "Effects",
    "UI",
    "Param",
    "Music",
    "Misc",
];

var currentState = MOD_MENU;


var LButtonHeld = false;
var RButtonHeld = false;
var AButtonHeld = false;
var BButtonHeld = false;

var currentDescHeight = 0; // Used for the current position of the description (modified by the R-Stick Y Value).
var currentActiveDescription // For reference to the current active description.
var activeDescHeight = 0; // The height for the current active description so it can't be scrolled out of bounds.

var mods = [];
var currentMods = [];
var pageCount = 0;

function createMod(mod, arrIdx) {
    var hidden = mod['is_disabled'] ? "hidden" : "";
    return `<button id="btn-mods-${mod['id']}" data-mod-index="${mod['id']}" data-current-mod-idx="${arrIdx}" tabindex="0" class="flex-button abstract-button All ${mod['category']}" nx-se-disabled="">
    <div class="abstract-icon-back-decoration"></div>
    <div class="abstract-button-border">
        <div class="abstract-button-inner">
            <div class="abstract-icon-wrapper">
                <div class="img-check ${hidden}">
                    <img class="abstract-icon is-appear" src="check.svg" />
                </div>
            </div>
            <div class="abstract-button-text f-u-bold mod-name"
                style="margin-top: 8px; display: block; font-size: 3vmin;" data-display_name="${mod['display_name']}">
                <span class="marquee" data-msgid="textbox_id-4-1">${mod['display_name']}</span>
            </div>
        </div>
    </div>
</button>`;
}

function createMods(mods) {
    var res = "";
    for (var i = 0; i < mods.length; i++) {
        res += createMod(mods[i], i);
    }
    return res;
}

function toggleMod() {
    var index = parseInt($(".is-focused").attr("data-mod-index"));
    var currentIndex = parseInt($(".is-focused").attr("data-current-mod-idx"));
    var checkContainer = $(".is-focused .img-check");
    checkContainer.toggleClass("hidden");
    var enabled = !checkContainer.hasClass("hidden");
    mods[index]["is_disabled"] = !enabled;
    currentMods[currentIndex]["is_disabled"] = !enabled;
    // Send mod index and status
    window.nx.sendMessage(JSON.stringify({
        "ToggleModRequest": {
            "id": index,
            "state": enabled
        }
    }));
}

function updateCurrentDesc() {
    // Reset current description height
    currentDescHeight = 0;

    // Assign the currently active description element to the global active description variable for use later
    currentActiveDescription = $('.l-main-content:not(.is-hidden)').eq(0).find(".l-description").eq(0);
    // Subtract 146 from the description scroll height to match the paragarph overflow
    activeDescHeight = currentActiveDescription[0].scrollHeight - 146;

    // Check to see if overflow occured and if so, enable the R-Stick Icon
    if (checkOverflow(currentActiveDescription[0])) {
        document.getElementById("r-stick-desc-icon").style.visibility = "visible";
    } else {
        document.getElementById("r-stick-desc-icon").style.visibility = "hidden";
    }
}

function checkGamepad(gamepad) {
    var axisX = gamepad.axes[0];
    var axisY = gamepad.axes[1];

    if (currentState == MOD_MENU) {

        if (gamepad.buttons[2].pressed) {
            showSubMenu();
        }

        if (gamepad.buttons[1].pressed) {
            if (!AButtonHeld) {
                toggleMod();
                AButtonHeld = true;
            }
        } else {
            AButtonHeld = false;
        }

        if (gamepad.buttons[0].pressed) {
            if (!BButtonHeld) {
                exit();
                BButtonHeld = true;
            }
        } else {
            BButtonHeld = false;
        }

        // Check if D-pad Left pressed or Left Stick X Axis less than -0.7
        if (gamepad.buttons[14].pressed || axisX < -0.7) {
            console.log("D-pad left pressed");
        }
        // Check if D-pad Up pressed or Y-Axis
        else if (gamepad.buttons[12].pressed || axisY < -0.7) {
            moveUp();
        }
        // Check if D-pad Right pressed or X Axis > 0.7
        else if (gamepad.buttons[15].pressed || axisX > 0.7) {
            console.log("D-pad Right pressed");
        }
        // Check if D-pad Down pressed or Y Axis > 0.7
        else if (gamepad.buttons[13].pressed || axisY > 0.7) {
            moveDown();
        };

        //#region L and R button Pressed (Pagination Prev or Next)
        if (gamepad.buttons[4].pressed) {
            prevPage();
        }

        if (gamepad.buttons[5].pressed) {
            nextPage();
        }
        //#endregion

        //#region R-Stick Y Value Calculation (Description scroll)
        var RStickYValue = gamepad.axes[3].toFixed(2);

        RStickYValue = (((RStickYValue - 0) * (20 - 0)) / (1 - 0)) + 0;
        currentDescHeight += RStickYValue;

        if (currentDescHeight < 0) {
            currentDescHeight = 0;
        } else if (currentDescHeight > activeDescHeight) {
            currentDescHeight = activeDescHeight;
        }
        //#endregion

        currentActiveDescription.scrollTop(currentDescHeight);
        //#endregion
    } else if (currentState == SUB_MENU) {
        // Handle sub menu controls

        // If B button pressed
        if (gamepad.buttons[0].pressed) {
            if (!BButtonHeld) {
                showModMenu();
                BButtonHeld = true;
            }
        } else {
            BButtonHeld = false;
        }
    }
}

function moveUp() {
    var source = document.querySelector("#mods>button.is-focused");
    var target = document.querySelector("#mods>button.is-focused").previousElementSibling;

    if (source == undefined) {
        target = document.querySelector("#mods>button:first-child");
    }

    if (target == undefined) {
        target = document.querySelector("#mods>button:last-child");
    }

    move(source, target);
}

function moveDown() {
    var source = document.querySelector("#mods>button.is-focused");
    var target = document.querySelector("#mods>button.is-focused").nextElementSibling;

    if (source == undefined) {
        target = document.querySelector("#mods>button:first-child");
    }

    if (target == undefined) {
        target = document.querySelector("#mods>button:first-child");
    }

    move(source, target);
}

function move(source, target) {
    if (source != undefined) {
        source.classList.remove("is-focused");
        var srcModName = $(source).find(".abstract-button-text");
        srcModName.html(`<span class="marquee" data-msgid="textbox_id-4-1">${srcModName.attr('data-display_name')}</span>`);
    }

    if (target != undefined) {
        var tgtModName = $(target).find(".marquee");

        if (checkOverflow(tgtModName[0])) {
            $(tgtModName).marquee({
                //speed milliseconds
                duration: 5000,
                //gap in pixels between the tickers
                gap: 400,
                //time in milliseconds before the marquee will start animating
                delayBeforeStart: 300,
                //'left' or 'right'
                direction: 'left',
                //true or false - should the marquee be duplicated to show an effect of continues flow
                duplicated: true,
                // should the text be visible before starting
                startVisible: true
            });
        }
        target.classList.add("is-focused");
        target.focus();
        var mod = mods[target.getAttribute("data-mod-index")];
        $("#description").html(mod["description"]);
        $("#version").html(mod["version"]);
        $("#authors").html(mod["authors"]);
        $("#preview").attr("src", `img/${mod['id']}`);
        updateCurrentDesc();
    }
}

function nextPage() {

    if ($('#mods').pagination("getTotalPage") <= 1) { return; }

    if ($('#mods').pagination("getSelectedPageNum") == $('#mods').pagination("getTotalPage")) {
        $('#mods').pagination("go", 1);
    } else {
        $('#mods').pagination("next");
    }
}

function prevPage() {

    if ($('#mods').pagination("getTotalPage") <= 1) { return; }

    if ($('#mods').pagination("getSelectedPageNum") == 1) {
        $('#mods').pagination("go", $('#mods').pagination("getTotalPage"));
    } else {
        $('#mods').pagination("previous");
    }
}

// yoinked from here https://stackoverflow.com/questions/143815/determine-if-an-html-elements-content-overflows
function checkOverflow(el) {
    var curOverflow = el.style.overflow;

    if (!curOverflow || curOverflow === "visible") {
        el.style.overflow = "hidden";
    }

    var isOverflowing = el.clientWidth < el.scrollWidth ||
        el.clientHeight < el.scrollHeight;

    el.style.overflow = curOverflow;

    return isOverflowing;
}

function showSubMenu() {
    $("#submenu").css("display", "flex");
    $("#Fighter").focus();
    document.querySelector('meta[name="focus-ring-visibility"]').setAttribute("content", "");
    currentState = SUB_MENU;
}

function showModMenu() {
    $("#submenu").css("display", "none");
    document.querySelector('meta[name="focus-ring-visibility"]').setAttribute("content", "hidden");
    var categoriesToUse = [];
    $('#filters input:checkbox:checked').each(function(idx) {
        categoriesToUse.push($(this).attr('id'));
    });
    currentMods = categoriesToUse.length == 0 ? mods : mods.filter(mod => categoriesToUse.includes(mod["category"]));
    currentMods = currentMods.length == 0 ? mods : currentMods;
    refreshCurrentMods();
    currentState = MOD_MENU;
}

function refreshCurrentMods() {
    updateSort();
    $('#mods').pagination({
        dataSource: currentMods,
        showPrevious: false,
        showNext: false,
        showPageNumbers: false,
        callback: function(data, pagination) {
            $("#mods").html(createMods(data));
            move(undefined, $("#mods>button").get(0));
            pageCount = Math.ceil(pagination["totalNumber"] / pagination["pageSize"]);
        },
        afterPaging: function(activePage) {
            $("#pageInfo").html(`${activePage} of ${pageCount}`);
        }
    });
}

function setAllState(state, src) {
    for (var i = 0; i < mods.length; i++) {
        mods[i]["is_disabled"] = !state;
    }
    refreshCurrentMods();
    src != undefined || src != null ? src.focus() : false;
    window.nx.sendMessage(JSON.stringify({
        "ChangeAllRequest": {
            "state": state
        }
    }));
}

function exit() {
    window.nx.sendMessage(JSON.stringify("ClosureRequest"));
    window.location.href = "http://localhost/quit";
}

function updateSort() {
    var descending = document.getElementById('desc').checked;
    var sortType = document.getElementById('sortOptions').value;

    if (sortType == "alphabetical") {
        currentMods = JSON.parse(JSON.stringify(currentMods)).sort((a, b) => {
            if (a["display_name"] < b["display_name"]) { return -1; }
            if (a["display_name"] > b["display_name"]) { return 1; }
            return 0;
        });
    }

    if (descending) {
        currentMods.reverse();
    }
}

window.addEventListener("DOMContentLoaded", (e) => {
    if (!isNx) {
        mods = [];
        for (var i = 0; i < 9999; i++) {
            mods.push({
                "id": i,
                "display_name": `Mod #${i}`,
                "version": `${i + 3}.${i + 2}.${i}`,
                "is_disabled": true,
                "category": categories[i % categories.length],
                "authors": `Coolsonickirby`,
                "description": `Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}.`,
            });
        }

        currentMods = mods;
        refreshCurrentMods();
    } else {

        $.ajax({
            dataType: "json",
            url: "mods.json",
            success: (data) => {
                mods = data;
                $("#workspace").html(mods.length);
                currentMods = mods;
                refreshCurrentMods();
            }
        });

        // Listen to the keydown event and prevent the default
        window.addEventListener('keydown', function(e) {
            if (currentState != SUB_MENU) {
                e.preventDefault();
            }
        });

        window.nx.footer.setAssign("B", "", () => {});
        window.nx.footer.setAssign("X", "", () => {});
    }
});
