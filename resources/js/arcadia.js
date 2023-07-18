const MOD_MENU = "modMenu";
const SUB_MENU = "subMenu";
const categories = [
    "All",
    "Fighter",
    "Stage",
    "Item",
    "UI",
    "Parameter",
    "Sound",
    "Plugin",
    "Miscellaneous",
];
var categoriesToUse = [];

var currentState = MOD_MENU;

var currentDescHeight = 0; // Used for the current position of the description (modified by the R-Stick Y Value).
var currentActiveDescription // For reference to the current active description.
var activeDescHeight = 0; // The height for the current active description so it can't be scrolled out of bounds.

var mods = [];
var currentMods = [];
var modSize = 0;
var pageCount = 0;

function createMod(mod_id) {
    var hidden = mods[mod_id]['is_disabled'] ? "hidden" : "";
    return `<button id="btn-mods-${mod_id}" data-mod-index="${mod_id}" tabindex="0" class="flex-button abstract-button All ${mods[mod_id]['category']}" nx-se-disabled="">
    <div class="abstract-icon-back-decoration"></div>
    <div class="abstract-button-border">
        <div class="abstract-button-inner">
            <div class="abstract-icon-wrapper">
                <div class="img-check ${hidden}">
                    <img class="abstract-icon is-appear" src="check.svg" />
                </div>
            </div>
            <div class="abstract-button-text f-u-bold mod-name"
                style="display: block; font-size: 26px; text-indent: 10px; margin-top: 8px;" data-display_name="${mods[mod_id]['display_name']}">
                <span class="marquee" data-msgid="textbox_id-4-1">${mods[mod_id]['display_name']}</span>
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
    var checkContainer = $(".is-focused .img-check");
    checkContainer.toggleClass("hidden");
    var enabled = !checkContainer.hasClass("hidden");
    mods[index]["is_disabled"] = !enabled;
    // Send mod index and status
    window.nx.sendMessage(JSON.stringify({
        "ToggleMod": {
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

function checkGamepad(index, gamepad) {
    var axisX = gamepad.axes[0];
    var axisY = gamepad.axes[1];

    if (currentState == MOD_MENU) {
        var RStickYValue = gamepad.axes[3].toFixed(2);

        RStickYValue = (((RStickYValue - 0) * (20 - 0)) / (1 - 0)) + 0;
        currentDescHeight += RStickYValue;

        if (currentDescHeight < 0) {
            currentDescHeight = 0;
        } else if (currentDescHeight > activeDescHeight) {
            currentDescHeight = activeDescHeight;
        }
        currentActiveDescription.scrollTop(currentDescHeight);
    }
}

function moveUp() {
    var source = document.querySelector("#mods>button.is-focused");
    var target = document.querySelector("#mods>button.is-focused").previousElementSibling;

    if (source == undefined) {
        target = document.querySelector("#mods>button:first-child");
    }

    if (target == undefined) {
        prevPage();
        target = document.querySelector("#mods>button:last-child");
        move(document.querySelector("#mods>button.is-focused"), target);
        return;
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
        nextPage();
        target = document.querySelector("#mods>button:first-child");
        move(document.querySelector("#mods>button.is-focused"), target);
        return;
    }

    move(source, target);
}

function move(source, target) {
    if (source != undefined && target != undefined) {
        if (source.id == target.id) {
            return;
        }
    }

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
        $("#author").html(mod["author"]);
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

function sizeToFormattedBytes(size) {
    if ((size / 1024) < 1)
        return `${size} bytes`; 
    size = size / 1024;

    if ((size / 1024) < 1)
        return `${size} kb`;
    size = size / 1024;

    if ((size / 1024) < 1)
        return `${size} mb`;
    size = size / 1024;

    return `${size.toFixed(2)} gb`;
}

function showSubMenu() {
    $("#modsCount").html(`${mods.length} mod${mods.length > 1 ? 's' : ''}`);
    var activeMods = 0;
    mods.forEach(mod => activeMods = mod["is_disabled"] ? activeMods : activeMods + 1);
    $("#activeModsCount").html(`${activeMods} active mod${activeMods > 1 ? 's' : ''}`);
    if (modSize == 0)
        $("#modSize").html("")
    else
        $("#modSize").html(`${sizeToFormattedBytes(modSize)} of mods enabled`)

    $("#submenu").css("display", "flex");
    $("#Fighter").focus();
    document.querySelector('meta[name="focus-ring-visibility"]').setAttribute("content", "");
    currentState = SUB_MENU;
}

function updateCurrentModsWCategories() {
    categoriesToUse = [];
    $('#filters input:checkbox:checked').each(function(idx) {
        categoriesToUse.push($(this).attr('id'));
    });
    currentMods = categoriesToUse.length == 0 ? mods.map(x => x["id"]) : mods.filter(mod => categoriesToUse.includes(mod["category"])).map(x => x["id"]);
}

function showModMenu() {
    $("#submenu").css("display", "none");
    document.querySelector('meta[name="focus-ring-visibility"]').setAttribute("content", "hidden");
    updateCurrentModsWCategories();
    if (currentMods.length == 0) {
        $("#description").html(`No mods found under:<br />${categoriesToUse.join("<br />")}`);
    }
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
        pageSize: 7,
        callback: function(data, pagination) {
            $("#mods").html(createMods(data));
            move(undefined, $("#mods>button").get(0));
            pageCount = Math.ceil(pagination["totalNumber"] / pagination["pageSize"]);
        },
        afterPaging: function(activePage) { 
            Array.from(document.querySelectorAll('.abstract-button')).forEach(item => {
                item.addEventListener('focus', event => {
                    // item.classList.add("is-focused");
                    move(undefined, item);
                });
                item.addEventListener('focusout', event => {
                    move(item, undefined);
                });
                item.addEventListener("click", event => {
                    toggleMod();
                });
            })
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
        "ChangeAll": {
            "state": state
        }
    }));
}

function setCurrentModsState(state, src) {
    updateCurrentModsWCategories();
    for (var i = 0; i < currentMods.length; i++) {
        mods[currentMods[i]]["is_disabled"] = !state;
    }
    refreshCurrentMods();
    src != undefined || src != null ? src.focus() : false;
    if (currentMods.length <= 0) { return; }
    window.nx.sendMessage(JSON.stringify({
        "ChangeIndexes": {
            "state": state,
            "indexes": currentMods
        }
    }));
}

function exit() {
    window.nx.sendMessage(JSON.stringify("Closure"));
    window.location.href = "http://localhost/quit";
}

function updateSort() {
    var descending = document.getElementById('desc').checked;
    var sortType = document.getElementById('sortOptions').value;

    if (sortType == "alphabetical") {
        currentMods = JSON.parse(JSON.stringify(currentMods)).sort((a, b) => {
            if (mods[a]["folder_name"] < mods[b]["folder_name"]) { return -1; }
            if (mods[a]["folder_name"] > mods[b]["folder_name"]) { return 1; }
            return 0;
        });
    } else if (sortType == "enabled") {
        currentMods = JSON.parse(JSON.stringify(currentMods)).sort((a, b) => {
            if (!mods[a]["is_disabled"] != !mods[b]["is_disabled"]) {
                return mods[b]["is_disabled"] ? -1 : 1;
            } else {
                if (mods[a]["folder_name"] < mods[b]["folder_name"]) { return -1; }
                if (mods[a]["folder_name"] > mods[b]["folder_name"]) { return 1; }
            }
            return 0;
        });
    } else if (sortType == "disabled") {
        currentMods = JSON.parse(JSON.stringify(currentMods)).sort((a, b) => {
            if (!mods[a]["is_disabled"] != !mods[b]["is_disabled"]) {
                return mods[b]["is_disabled"] ? 1 : -1;
            } else {
                if (mods[a]["folder_name"] < mods[b]["folder_name"]) { return -1; }
                if (mods[a]["folder_name"] > mods[b]["folder_name"]) { return 1; }
            }
            return 0;
        });
    }

    if (descending) {
        currentMods.reverse();
    }
}

window.nx.addEventListener("message", (e) => {
    var info = JSON.parse(e.data);
    if (!("mod_size" in info))
        return;
    
    modSize = info["mod_size"];
});

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
                "author": `Coolsonickirby`,
                "description": `Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}. Hey guys! This is one of the coolest mods ever made! Mod #${i}.`,
            });
        }

        currentMods = mods.map(x => x["id"]);
        refreshCurrentMods();
    } else {

        $.ajax({
            dataType: "json",
            url: "mods.json",
            success: (data) => {
                mods = data["entries"];
                $("#workspace").html(data["workspace"]);
                currentMods = mods.map(x => x["id"]);
                refreshCurrentMods();
            }
        });

        // Listen to the keydown event and prevent the
        // default
        window.addEventListener('keydown', function(e) {
            if (e.keyCode == UP) {
                var target = document.querySelector("#mods>button.is-focused").previousElementSibling;
                if (target == undefined) {
                    move(document.querySelector("#mods>button.is-focused"), undefined);
                    prevPage();
                    target = document.querySelector("#mods>button:last-child");
                    target.focus();
                }
            } else if (e.keyCode == DOWN) {
                var target = document.querySelector("#mods>button.is-focused").nextElementSibling;

                if (target == undefined) {
                    move(document.querySelector("#mods>button.is-focused"), undefined);
                    nextPage();
                    target = document.querySelector("#mods>button:first-child");
                    target.focus();
                }
            }
        });

        window.nx.footer.setAssign("X", "", () => {});
        window.nx.footer.setAssign("B", "", () => {
            if (currentState == SUB_MENU) {
                showModMenu();
            } else {
                exit();
            }
        });
        window.nx.footer.setAssign("Y", "", () => {
            if (currentState == MOD_MENU) {
                showSubMenu();
            }
        });
        window.nx.footer.setAssign("L", "", () => {
            if (currentState == MOD_MENU) {
                prevPage();
            }
        });
        window.nx.footer.setAssign("R", "", () => {
            if (currentState == MOD_MENU) {
                nextPage();
            }
        });
        window.nx.sendMessage(JSON.stringify("GetModSize"));
    }
});

// window.nx.sendMessage(JSON.stringify("GetModSize"));