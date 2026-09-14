
var WORKSPACES_CONTROL = "&#xe000 Set Active &nbsp; &#xe002 Duplicate Workspace &nbsp; &#xe003 Show Options";
var WORKSPACE_CONTROL = "&#xe000 Select Option";

var workspaces = [];
var activeWorkspace = "";
var selected = 0;
var currentPanel = "workspaces";
var dom = {};

function selectedName() {
    return workspaces[selected];
}

function focusedIndex() {
    var focused = focusedButton();
    var id = focused ? parseInt(focused.getAttribute("data-id"), 10) : NaN;
    return isNaN(id) ? -1 : id;
}

function renderWorkspaces() {
    workspaces.sort(function(a, b) {
        return a.localeCompare(b);
    });

    dom.container.innerHTML = "";
    for (var i = 0; i < workspaces.length; i++) {
        var button = document.createElement("button");
        button.className = "flex-item";
        button.setAttribute("data-id", i);
        button.innerHTML =
            '<div class="icon-background"><img class="abstract-icon is-appear" src="check.svg" /></div>' +
            '<div class="item-container"><h2></h2></div>';
        button.querySelector("img").style.display = workspaces[i] === activeWorkspace ? "block" : "none";
        button.querySelector("h2").textContent = workspaces[i];
        button.onclick = pickAndActivate;
        dom.container.appendChild(button);
    }

    var create = document.createElement("button");
    create.className = "flex-item";
    create.innerHTML = '<div class="icon-background"></div><div class="item-container"><h2>Create Workspace</h2></div>';
    create.onclick = createWorkspace;
    dom.container.appendChild(create);
}

function pickAndActivate() {
    selected = parseInt(this.getAttribute("data-id"), 10);
    setActive();
}

function goBack() {
    if (currentPanel === "workspaceOption") {
        showList();
    } else {
        exit();
    }
}

function exit() {
    send("ClosureRequest");
    window.location.href = "http://localhost/quit";
}

function showList() {
    dom.arrow.style.display = "none";
    dom.workspace.style.display = "none";
    dom.message.innerHTML = WORKSPACES_CONTROL;
    switchPanel(dom.optionPanel, dom.listPanel, function() {
        currentPanel = "workspaces";
        renderWorkspaces();
        focusButton(dom.listPanel, selected);
    });
}

function showWorkspace(index) {
    selected = index;
    var isDefault = selectedName() === "Default";
    dom.removeButton.style.display = isDefault ? "none" : "";
    dom.renameButton.style.display = isDefault ? "none" : "";
    dom.activeCheck.style.display = selectedName() === activeWorkspace ? "block" : "none";

    dom.arrow.style.display = "";
    dom.workspace.textContent = selectedName();
    dom.workspace.style.display = "";
    dom.message.innerHTML = WORKSPACE_CONTROL;
    switchPanel(dom.listPanel, dom.optionPanel, function() {
        currentPanel = "workspaceOption";
        focusButton(dom.optionPanel, 0);
    });
}

function setActive() {
    activeWorkspace = selectedName();
    if (currentPanel === "workspaceOption") {
        dom.activeCheck.style.display = "block";
    } else {
        var buttons = dom.container.querySelectorAll("button[data-id]");
        for (var i = 0; i < buttons.length; i++) {
            buttons[i].querySelector("img").style.display = i === selected ? "block" : "none";
        }
    }
    send({ "SetActive": { "name": activeWorkspace } });
}

function nameTaken(name) {
    return workspaces.indexOf(name) >= 0;
}

function renameWorkspace() {
    if (selectedName() === "Default") {
        return;
    }
    var name = prompt("Rename workspace", selectedName());
    if (name === null || name === undefined || name === "") {
        return;
    }
    if (nameTaken(name)) {
        alert("Workspace with that name already exists!");
        return;
    }

    var sourceName = selectedName();
    workspaces[selected] = name;
    dom.workspace.textContent = name;
    send({ "Rename": { "source_name": sourceName, "target_name": name } });
    if (activeWorkspace === sourceName) {
        setActive();
    }
}

function duplicateWorkspace() {
    var name = prompt("Name for duplicated workspace", selectedName());
    if (name === null || name === undefined || name === "") {
        return false;
    }
    if (nameTaken(name)) {
        alert("Workspace with that name already exists!");
        return false;
    }

    send({ "Duplicate": { "source_name": selectedName(), "target_name": name } });
    workspaces.push(name);
    return true;
}

function duplicateFromList() {
    var index = focusedIndex();
    if (index < 0) {
        return;
    }
    selected = index;
    var source = selectedName();
    if (duplicateWorkspace()) {
        renderWorkspaces();
        selected = workspaces.indexOf(source);
        focusButton(dom.listPanel, selected);
    }
}

function removeWorkspace() {
    if (selectedName() === "Default") {
        return;
    }
    if (!confirm("Do you really want to delete workspace " + selectedName() + "?")) {
        return;
    }
    if (!confirm("Are you really sure you want to delete workspace " + selectedName() + "?")) {
        return;
    }
    send({ "Remove": { "name": selectedName() } });
    workspaces.splice(selected, 1);
    selected = 0;
    showList();
}

function createWorkspace() {
    var name = prompt("Enter new workspace name");
    if (name === null || name === undefined || name === "") {
        return;
    }
    if (nameTaken(name)) {
        alert("Workspace with that name already exists!");
        return;
    }

    workspaces.push(name);
    send({ "Create": { "name": name } });
    renderWorkspaces();
    selected = workspaces.indexOf(name);
    focusButton(dom.listPanel, selected);
}

function editWorkspace() {
    send({ "Edit": { "name": selectedName() } });
    if (isNx) {
        window.location.href = "http://localhost/quit";
    }
}

window.addEventListener("DOMContentLoaded", function() {
    dom.listPanel = document.getElementById("workspaces");
    dom.optionPanel = document.getElementById("workspaceOption");
    dom.container = document.getElementById("workspacesContainer");
    dom.arrow = document.getElementById("workspaceArrow");
    dom.workspace = document.getElementById("workspace");
    dom.message = document.getElementById("message");
    dom.activeCheck = document.querySelector("#is-active img");
    dom.renameButton = document.getElementById("renameWorkspace");
    dom.removeButton = document.getElementById("removeWorkspace");

    if (typeof WORKSPACES_DATA !== "undefined") {
        workspaces = WORKSPACES_DATA.workspaces;
        activeWorkspace = WORKSPACES_DATA.active_workspace;
    } else {
        for (var i = 0; i < 10; i++) {
            workspaces.push("Workspace #" + (i + 1));
        }
        activeWorkspace = workspaces[0];
    }

    trackButtonFocus();
    installListNavigation();

    if (isNx) {
        window.nx.footer.setAssign("A", "", function() {
            var focused = focusedButton();
            if (focused) {
                focused.click();
            } else {
                focusButton(currentPanel === "workspaces" ? dom.listPanel : dom.optionPanel, 0);
            }
        });
        window.nx.footer.setAssign("B", "", goBack);
        window.nx.footer.setAssign("X", "", function() {
            if (currentPanel === "workspaces") {
                duplicateFromList();
            }
        });
        window.nx.footer.setAssign("Y", "", function() {
            if (currentPanel === "workspaces") {
                var index = focusedIndex();
                if (index >= 0) {
                    showWorkspace(index);
                }
            }
        });
    }

    renderWorkspaces();
    focusButton(dom.listPanel, 0);
});
