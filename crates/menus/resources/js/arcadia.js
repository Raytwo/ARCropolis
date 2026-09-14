
var MOD_MENU = "modMenu";
var SUB_MENU = "subMenu";
var PAGE_SIZE = 7;
var KEY_UP = 38;
var KEY_DOWN = 40;
var PREVIEW_DELAY = 120;
var MARQUEE_SPEED = 80;
var DESC_SCROLL_STEP = 14;
var STICK_DEAD_ZONE = 0.15;

var isNx = typeof window.nx !== "undefined";

var mods = [];
var order = [];
var page = 0;
var state = MOD_MENU;
var buttons = [];
var focusedButton = null;
var previewTimer = null;
var descScroll = 0;
var descMax = 0;
var dom = {};

function send(message) {
    if (isNx) {
        window.nx.sendMessage(JSON.stringify(message));
    }
}

function plural(count, word) {
    return count + " " + word + (count == 1 ? "" : "s");
}

function pageCount() {
    return Math.ceil(order.length / PAGE_SIZE);
}

function visibleCount() {
    return Math.min(PAGE_SIZE, order.length - page * PAGE_SIZE);
}

function buildButton(slot) {
    var el = document.createElement("button");
    el.className = "flex-button abstract-button";
    el.tabIndex = 0;
    el.setAttribute("nx-se-disabled", "");
    el.innerHTML =
        '<div class="abstract-icon-back-decoration"></div>' +
        '<div class="abstract-button-border"><div class="abstract-button-inner">' +
        '<div class="abstract-icon-wrapper"><div class="img-check"><img class="abstract-icon is-appear" src="check.svg" /></div></div>' +
        '<div class="abstract-button-text f-u-bold mod-name" style="display: block; font-size: 26px; text-indent: 10px; margin-top: 8px;">' +
        '<span class="marquee"><div class="marquee-text"></div></span>' +
        '</div></div></div>';

    var button = {
        el: el,
        slot: slot,
        id: -1,
        check: el.querySelector(".img-check"),
        marquee: el.querySelector(".marquee"),
        text: el.querySelector(".marquee-text"),
        marqueeToken: 0
    };
    el.arcadiaSlot = slot;
    var ended = function() {
        onMarqueeEnd(button);
    };
    button.text.addEventListener("transitionend", ended);
    button.text.addEventListener("webkitTransitionEnd", ended);
    return button;
}

function buttonFor(target) {
    while (target && target !== dom.mods) {
        if (target.arcadiaSlot !== undefined) {
            return buttons[target.arcadiaSlot];
        }
        target = target.parentNode;
    }
    return null;
}

function setCheck(button, mod) {
    button.check.className = mod.is_disabled ? "img-check hidden" : "img-check";
}

function renderPage() {
    var start = page * PAGE_SIZE;
    for (var slot = 0; slot < PAGE_SIZE; slot++) {
        var button = buttons[slot];
        var id = start + slot < order.length ? order[start + slot] : -1;
        button.id = id;
        stopMarquee(button);
        if (id < 0) {
            button.el.style.display = "none";
            continue;
        }
        var mod = mods[id];
        button.el.style.display = "";
        button.el.className = "flex-button abstract-button All " + mod.category + (button === focusedButton ? " is-focused" : "");
        setCheck(button, mod);
        button.text.textContent = mod.display_name;
    }
    var total = pageCount();
    dom.pageInfo.textContent = total > 0 ? (page + 1) + " of " + total : "";
}

function focusSlot(slot) {
    var button = buttons[slot];
    button.el.focus();
    onFocus(button);
}

function onFocus(button) {
    if (button.id < 0) {
        return;
    }
    if (focusedButton && focusedButton !== button) {
        onBlur(focusedButton);
    }
    focusedButton = button;
    button.el.classList.add("is-focused");

    var mod = mods[button.id];
    dom.version.textContent = mod.version;
    dom.authors.textContent = mod.authors;
    dom.description.innerHTML = mod.description;
    resetDescription();
    startMarquee(button);
    schedulePreview(mod);
}

function onBlur(button) {
    if (focusedButton === button) {
        focusedButton = null;
    }
    button.el.classList.remove("is-focused");
    stopMarquee(button);
}

function clearDetails() {
    dom.version.textContent = "";
    dom.authors.textContent = "";
    dom.description.innerHTML = "";
    resetDescription();
    schedulePreview(null);
}

function schedulePreview(mod) {
    if (previewTimer !== null) {
        clearTimeout(previewTimer);
    }
    previewTimer = setTimeout(function() {
        previewTimer = null;
        var src = mod && mod.image ? mod.image : "missing.webp";
        if (dom.preview.getAttribute("src") !== src) {
            dom.preview.setAttribute("src", src);
        }
    }, PREVIEW_DELAY);
}

function resetDescription() {
    descScroll = 0;
    dom.descBox.scrollTop = 0;
    descMax = Math.max(0, dom.descBox.scrollHeight - dom.descBox.clientHeight);
    dom.descIcon.style.visibility = descMax > 0 ? "visible" : "hidden";
}

function textWidth(el) {
    var width = Math.max(el.offsetWidth, el.scrollWidth);
    if (document.createRange && el.firstChild) {
        var range = document.createRange();
        range.selectNodeContents(el);
        var rect = range.getBoundingClientRect ? range.getBoundingClientRect() : null;
        if (rect && rect.width > width) {
            width = rect.width;
        }
    }
    return width;
}

function startMarquee(button) {
    var distance = Math.ceil(textWidth(button.text) - button.marquee.clientWidth);
    if (distance <= 0) {
        return;
    }
    var token = ++button.marqueeToken;
    button.marqueeDistance = distance;
    setTimeout(function() {
        if (button.marqueeToken === token) {
            slideMarquee(button);
        }
    }, 500);
}

function setTransform(el, value, duration) {
    el.style.webkitTransitionDuration = duration;
    el.style.transitionDuration = duration;
    el.style.webkitTransform = value;
    el.style.transform = value;
}

function slideMarquee(button) {
    setTransform(button.text, "translateX(" + (-button.marqueeDistance) + "px)", (button.marqueeDistance / MARQUEE_SPEED) + "s");
}

function onMarqueeEnd(button) {
    var token = button.marqueeToken;
    setTimeout(function() {
        if (button.marqueeToken !== token) {
            return;
        }
        setTransform(button.text, "translateX(0)", "0s");
        setTimeout(function() {
            if (button.marqueeToken === token) {
                slideMarquee(button);
            }
        }, 1000);
    }, 1000);
}

function stopMarquee(button) {
    button.marqueeToken++;
    setTransform(button.text, "", "0s");
}

function nextPage(focusLast) {
    if (pageCount() > 1) {
        page = (page + 1) % pageCount();
        renderPage();
    }
    focusSlot(focusLast ? visibleCount() - 1 : 0);
}

function prevPage(focusLast) {
    if (pageCount() > 1) {
        page = (page + pageCount() - 1) % pageCount();
        renderPage();
    }
    focusSlot(focusLast ? visibleCount() - 1 : 0);
}

function toggleMod(button) {
    if (button.id < 0) {
        return;
    }
    var mod = mods[button.id];
    mod.is_disabled = !mod.is_disabled;
    setCheck(button, mod);
    send({ "ToggleMod": { "id": button.id, "state": !mod.is_disabled } });
}

function selectedCategories() {
    var boxes = document.querySelectorAll("#filters input:checked");
    var categories = [];
    for (var i = 0; i < boxes.length; i++) {
        categories.push(boxes[i].id);
    }
    return categories;
}

function compareNames(a, b) {
    var x = mods[a].display_name;
    var y = mods[b].display_name;
    return x < y ? -1 : (x > y ? 1 : 0);
}

function sortOrder() {
    var type = dom.sortOptions.value;
    if (type == "alphabetical") {
        order.sort(compareNames);
    } else {
        var disabledLast = type == "enabled" ? 1 : -1;
        order.sort(function(a, b) {
            var da = mods[a].is_disabled;
            var db = mods[b].is_disabled;
            if (da !== db) {
                return (da ? 1 : -1) * disabledLast;
            }
            return compareNames(a, b);
        });
    }
    if (dom.descending.checked) {
        order.reverse();
    }
}

function refresh() {
    var categories = selectedCategories();
    order = [];
    for (var i = 0; i < mods.length; i++) {
        if (categories.length == 0 || categories.indexOf(mods[i].category) >= 0) {
            order.push(i);
        }
    }
    sortOrder();
    page = Math.min(page, Math.max(0, pageCount() - 1));
    renderPage();

    if (order.length > 0) {
        focusSlot(0);
    } else {
        clearDetails();
        dom.description.innerHTML = categories.length > 0 ? "No mods found under:<br />" + categories.join("<br />") : "No mods found";
    }
}

function updateCounts() {
    var active = 0;
    for (var i = 0; i < mods.length; i++) {
        if (!mods[i].is_disabled) {
            active++;
        }
    }
    dom.modsCount.textContent = plural(mods.length, "mod");
    dom.activeModsCount.textContent = plural(active, "active mod");
}

function showSubMenu() {
    updateCounts();
    dom.submenu.style.display = "flex";
    dom.focusRing.setAttribute("content", "");
    document.getElementById("Fighter").focus();
    state = SUB_MENU;
}

function showModMenu() {
    dom.submenu.style.display = "none";
    dom.focusRing.setAttribute("content", "hidden");
    state = MOD_MENU;
    refresh();
}

function setAllState(enabled) {
    for (var i = 0; i < mods.length; i++) {
        mods[i].is_disabled = !enabled;
    }
    updateCounts();
    send({ "ChangeAll": { "state": enabled } });
}

function setCategoriesState(enabled) {
    var categories = selectedCategories();
    var touched = 0;
    for (var i = 0; i < mods.length; i++) {
        if (categories.length == 0 || categories.indexOf(mods[i].category) >= 0) {
            mods[i].is_disabled = !enabled;
            touched++;
        }
    }
    updateCounts();
    if (touched > 0) {
        send({ "ChangeCategories": { "state": enabled, "categories": categories } });
    }
}

function exit() {
    send("Closure");
    window.location.href = "http://localhost/quit";
}

function pollGamepad() {
    if (state !== MOD_MENU || descMax === 0) {
        return;
    }
    var pads = navigator.getGamepads();
    var pad = pads ? pads[0] : null;
    if (!pad) {
        return;
    }
    var y = pad.axes[3];
    if (y > -STICK_DEAD_ZONE && y < STICK_DEAD_ZONE) {
        return;
    }
    descScroll = Math.min(descMax, Math.max(0, descScroll + y * DESC_SCROLL_STEP));
    dom.descBox.scrollTop = descScroll;
}

function onKeyDown(e) {
    if (state !== MOD_MENU || focusedButton === null) {
        return;
    }
    if (e.keyCode == KEY_UP && focusedButton.slot == 0) {
        e.preventDefault();
        prevPage(true);
    } else if (e.keyCode == KEY_DOWN && focusedButton.slot == visibleCount() - 1) {
        e.preventDefault();
        nextPage(false);
    }
}

function loadSvgIcons() {
    var containers = document.querySelectorAll(".svg-container");
    for (var i = 0; i < containers.length; i++) {
        (function(container) {
            var xhr = new XMLHttpRequest();
            xhr.open("GET", container.getAttribute("ref"), true);
            xhr.onload = function() {
                container.innerHTML = xhr.responseText;
                container.classList.add("is-appear");
            };
            xhr.send();
        })(containers[i]);
    }
}

function dummyMods(count) {
    var categories = ["Fighter", "Stage", "Effects", "UI", "Param", "Audio", "Misc"];
    var list = [];
    for (var i = 0; i < count; i++) {
        list.push({
            "id": i,
            "display_name": (i % 5 == 0 ? "A very long mod name that needs to scroll to be read " : "Mod #") + i,
            "version": (i + 3) + "." + (i + 2) + "." + i,
            "is_disabled": i % 3 == 0,
            "category": categories[i % categories.length],
            "authors": "Coolsonickirby",
            "description": "Hey guys! This is one of the coolest mods ever made! Mod #" + i + ". ".repeat(i % 9 + 1),
            "image": null
        });
    }
    return list;
}

window.addEventListener("DOMContentLoaded", function() {
    dom.mods = document.getElementById("mods");
    dom.pageInfo = document.getElementById("pageInfo");
    dom.version = document.getElementById("version");
    dom.authors = document.getElementById("authors");
    dom.description = document.getElementById("description");
    dom.descBox = document.querySelector("#about-mods .l-description");
    dom.descIcon = document.getElementById("r-stick-desc-icon");
    dom.preview = document.getElementById("preview");
    dom.submenu = document.getElementById("submenu");
    dom.sortOptions = document.getElementById("sortOptions");
    dom.descending = document.getElementById("desc");
    dom.modsCount = document.getElementById("modsCount");
    dom.activeModsCount = document.getElementById("activeModsCount");
    dom.workspace = document.getElementById("workspace");
    dom.focusRing = document.querySelector('meta[name="focus-ring-visibility"]');

    for (var slot = 0; slot < PAGE_SIZE; slot++) {
        buttons.push(buildButton(slot));
        dom.mods.appendChild(buttons[slot].el);
    }

    dom.mods.addEventListener("focus", function(e) {
        var button = buttonFor(e.target);
        if (button) {
            onFocus(button);
        }
    }, true);
    dom.mods.addEventListener("blur", function(e) {
        var button = buttonFor(e.target);
        if (button) {
            onBlur(button);
        }
    }, true);
    dom.mods.addEventListener("click", function(e) {
        var button = buttonFor(e.target);
        if (button) {
            toggleMod(button);
        }
    });

    if (typeof ARCADIA_DATA !== "undefined") {
        mods = ARCADIA_DATA.entries;
        dom.workspace.textContent = ARCADIA_DATA.workspace;
    } else {
        mods = dummyMods(300);
    }

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("load", function() {
        if (focusedButton !== null) {
            startMarquee(focusedButton);
        }
    });

    if (isNx) {
        window.addEventListener("NXFirstPaintEndAfterLoad", function() {
            setTimeout(loadSvgIcons, 0);
        });
        window.addEventListener("gamepadconnected", function() {
            setInterval(pollGamepad, 100);
        });

        window.nx.footer.setAssign("X", "", function() {});
        window.nx.footer.setAssign("B", "", function() {
            if (state == SUB_MENU) {
                showModMenu();
            } else {
                exit();
            }
        });
        window.nx.footer.setAssign("Y", "", function() {
            if (state == MOD_MENU) {
                showSubMenu();
            }
        });
        window.nx.footer.setAssign("L", "", function() {
            if (state == MOD_MENU) {
                prevPage(false);
            }
        });
        window.nx.footer.setAssign("R", "", function() {
            if (state == MOD_MENU) {
                nextPage(false);
            }
        });
    } else {
        setTimeout(loadSvgIcons, 0);
    }

    refresh();
});
