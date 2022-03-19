var isNx = (typeof window.nx !== 'undefined')

// Check the gamepad input for saving, switching categories, and scrolling the description
function checkGamepad(gamepad) {
    //#region UI Input Check

    var axisX = gamepad.axes[0];
    var axisY = gamepad.axes[1];

    // Check A button
    if (gamepad.buttons[1].pressed) {
        console.log("A Button Pressed");
    }

    // Check if D-pad Left pressed or Left Stick X Axis less than -0.7
    if (gamepad.buttons[14].pressed || axisX < -0.7) {
        console.log("D-pad left pressed");
    }
    // Check if D-pad Up pressed or Y-Axis
    else if (gamepad.buttons[12].pressed || axisY < -0.7) {
        console.log("D-pad up pressed");
    }
    // Check if D-pad Right pressed or X Axis > 0.7
    else if (gamepad.buttons[15].pressed || axisX > 0.7) {
        console.log("D-pad Right pressed");
    }
    // Check if D-pad Down pressed or Y Axis > 0.7
    else if (gamepad.buttons[13].pressed || axisY > 0.7) {
        console.log("D-pad Down pressed");
    };
    //#endregion


    //#region + Button Pressed Check (Save)
    if (gamepad.buttons[9].pressed) {
        console.log("+ (Plus) button pressed");
    }
    //#endregion

    //#region R-Stick Y Value Calculation (Description scroll)
    var RStickYValue = gamepad.axes[3].toFixed(2);
    //#endregion

    //#region L and R button Pressed Checkd (Category Switching)
    if (gamepad.buttons[4].pressed) {
        console.log("L Button Pressed");
    }

    if (gamepad.buttons[5].pressed) {
        console.log("R Button Pressed");
    }
    //#endregion
}

function loadSVG() {
    $('.svg-container').each(function() {
        var $thisObj = $(this);
        $(this).load($thisObj.attr("ref"));
        $(this).addClass("is-appear");
    });
}

window.addEventListener("DOMContentLoaded", (e) => {
    // Listen to the gamepadconnected event
    window.addEventListener("gamepadconnected", function(e) {
        // Once a gamepad has connected, start an interval function that will run every 100ms to check for input
        setInterval(function() {
            // Check Player 1 Input
            checkGamepad(navigator.getGamepads()[0]);
        }, 150);
    });

    window.addEventListener('NXFirstPaintEndAfterLoad', function() {
        setTimeout(loadSVG, 0);
    });

    $(function() {
        if (!isNx) {
            setTimeout(loadSVG, 0)
        }
    })
});