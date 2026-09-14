
var UP = 38;
var DOWN = 40;
var FADE_MS = 200;
var isNx = typeof window.nx !== "undefined";

function send(message) {
    if (isNx) {
        window.nx.sendMessage(JSON.stringify(message));
    }
}

function focusedButton() {
    return document.querySelector("button.is-focused");
}

function trackButtonFocus() {
    document.addEventListener("focus", function(e) {
        if (e.target && e.target.tagName === "BUTTON") {
            e.target.classList.add("is-focused");
        }
    }, true);
    document.addEventListener("blur", function(e) {
        if (e.target && e.target.tagName === "BUTTON") {
            e.target.classList.remove("is-focused");
        }
    }, true);
}

function installListNavigation() {
    window.addEventListener("keydown", function(e) {
        if (e.keyCode !== UP && e.keyCode !== DOWN) {
            return;
        }
        var focused = focusedButton();
        if (!focused) {
            return;
        }
        var target = e.keyCode === UP ? focused.previousElementSibling : focused.nextElementSibling;
        if (target) {
            target.focus();
        }
    });
}

function visibleButtons(panel) {
    var all = panel.querySelectorAll("button");
    var shown = [];
    for (var i = 0; i < all.length; i++) {
        if (all[i].style.display !== "none") {
            shown.push(all[i]);
        }
    }
    return shown;
}

function focusButton(panel, index) {
    var buttons = visibleButtons(panel);
    if (buttons.length === 0) {
        return;
    }
    buttons[Math.min(Math.max(index, 0), buttons.length - 1)].focus();
}

function switchPanel(from, to, done) {
    from.style.opacity = "0";
    setTimeout(function() {
        from.style.display = "none";
        to.style.opacity = "0";
        to.style.display = "block";
        void to.offsetWidth;
        to.style.opacity = "1";
        setTimeout(done, FADE_MS);
    }, FADE_MS);
}
