
var panels = {};
var currentPanel = "settings";

function setCheck(id, on) {
    var img = document.getElementById(id);
    if (!img) {
        return;
    }
    if (on) {
        img.classList.remove("hidden");
    } else {
        img.classList.add("hidden");
    }
}

function applyData() {
    var flags = CONFIG_DATA.flags;
    for (var name in flags) {
        if (flags.hasOwnProperty(name)) {
            setCheck(name, flags[name]);
        }
    }
    setCheck(CONFIG_DATA.logging_level, true);
}

function toggleFlag(name) {
    CONFIG_DATA.flags[name] = !CONFIG_DATA.flags[name];
    setCheck(name, CONFIG_DATA.flags[name]);
    send({ "ToggleFlag": { "name": name } });
}

function setLoggingLevel(level) {
    if (level === CONFIG_DATA.logging_level) {
        return;
    }
    setCheck(CONFIG_DATA.logging_level, false);
    CONFIG_DATA.logging_level = level;
    setCheck(level, true);
    send({ "SetLoggingLevel": { "level": level } });
}

function showLogging() {
    switchPanel(panels.settings, panels.logging, function() {
        currentPanel = "logging";
        var current = panels.logging.querySelector('button[data-level="' + CONFIG_DATA.logging_level + '"]');
        if (current) {
            current.focus();
        } else {
            focusButton(panels.logging, 0);
        }
    });
}

function showSettings() {
    switchPanel(panels.logging, panels.settings, function() {
        currentPanel = "settings";
        focusButton(panels.settings, 0);
    });
}

function exit() {
    send("Closure");
    window.location.href = "http://localhost/quit";
}

window.addEventListener("DOMContentLoaded", function() {
    panels.settings = document.getElementById("settings");
    panels.logging = document.getElementById("logging");

    if (typeof CONFIG_DATA === "undefined") {
        window.CONFIG_DATA = { "flags": { "auto_update": true }, "logging_level": "Warn" };
    }
    applyData();

    trackButtonFocus();
    installListNavigation();

    if (isNx) {
        window.nx.footer.setAssign("X", "", function() {});
        window.nx.footer.setAssign("Y", "", function() {});
        window.nx.footer.setAssign("B", "", function() {
            if (currentPanel === "logging") {
                showSettings();
            } else {
                exit();
            }
        });
    }

    focusButton(panels.settings, 0);
});
