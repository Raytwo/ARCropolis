var AButtonHeld = false;

window.addEventListener("DOMContentLoaded", (e) => {
    var buttons = document.querySelectorAll('button');

    [].forEach.call(buttons, function(btn) {
        btn.addEventListener("focus", () => {
            btn.classList.add("is-focused");
        });

        btn.addEventListener("focusout", () => {
            btn.classList.remove("is-focused");
        });
    });

    // Listen to the keydown event and prevent the default
    window.addEventListener('keydown', function(e) {
        e.preventDefault();
    });

    // Listen to the gamepadconnected event
    window.addEventListener("gamepadconnected", function(e) {
        if ($(".is-focused").length <= 0) {
            $("#list").find("button").get(0).focus();
        }
    });
});

function checkGamepad(index, gamepad) {
    //#region UI Input Check

    var axisX = gamepad.axes[0];
    var axisY = gamepad.axes[1];

    // Check A button
    if (gamepad.buttons[1].pressed) {
        if (!AButtonHeld[index]) {
            AButtonHeld[index] = true;
            $(".is-focused").last().click();
        }
    } else {
        AButtonHeld[index] = false;
    }

    // Check if D-pad Left pressed or Left Stick X Axis less than -0.7
    if (gamepad.buttons[14].pressed || axisX < -0.7) {
        // Go up by 6 elements
        var slice_index = 6;
        var target = $(".is-focused").prevAll(":visible").slice(0, slice_index).last();
        while (target.length <= 0 && slice_index != 0) {
            slice_index -= 1;
            target = $(".is-focused").prevAll(":visible").slice(0, slice_index).last();
        }

        // If that doesn't exist, then dip
        if (target.length <= 0) {
            return;
        }
        scroll(target, $("#list").scrollTop() + target.position().top - 50, $("#list"));
    }
    // Check if D-pad Up pressed or Y-Axis
    else if (gamepad.buttons[12].pressed || axisY < -0.7) {
        // Get the mod above the current focused one
        var target = $(".is-focused").prev();

        while (target.length > 0 && target.is(':hidden')) {
            target = target.prev();
        }

        // If that doesn't exist, then dip
        if (target.length <= 0) {
            return;
        }

        scroll(target, $("#list").scrollTop() + target.position().top - 50, $("#list"));
    }
    // Check if D-pad Right pressed or X Axis > 0.7
    else if (gamepad.buttons[15].pressed || axisX > 0.7) {
        // Go up down 6 elements
        var slice_index = 6;
        var target = $(".is-focused").nextAll(":visible").slice(0, slice_index).last();

        while (target.length <= 0 && slice_index != 0) {
            slice_index -= 1;
            target = $(".is-focused").nextAll(":visible").slice(0, slice_index).last();
        }

        // If that doesn't exist, then dip
        if (target.length <= 0) {
            return;
        }

        scroll(target, ($("#list").scrollTop()) + (target.height() * 2), $("#list"));
    }
    // Check if D-pad Down pressed or Y Axis > 0.7
    else if (gamepad.buttons[13].pressed || axisY > 0.7) {
        // Get the next mod that will be focused on
        var target = $(".is-focused").next();

        while (target.length > 0 && target.is(':hidden')) {
            target = target.next();
        }

        console.log(target);
        // If there is none after that, then just return
        if (target.length <= 0) {
            return;
        }
        console.log(target);
        scroll(target, ($("#list").scrollTop()) + (target.height() * 2), $("#list"));
    };
    //#endregion
}

// Code to handle this session wasn't made to detect a closure by button
// window.nx.footer.unsetAssign( "B" );