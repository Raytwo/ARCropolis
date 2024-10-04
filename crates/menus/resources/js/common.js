const UP = 38;
const DOWN = 40;
var isNx = (typeof window.nx !== 'undefined');

// Example of checkGamepad Function
// function checkGamepad(gamepad) {
//     //#region UI Input Check

//     var axisX = gamepad.axes[0];
//     var axisY = gamepad.axes[1];

//     // Check A button
//     if (gamepad.buttons[1].pressed) {
//         console.log("A Button Pressed");
//     }

//     // Check if D-pad Left pressed or Left Stick X Axis less than -0.7
//     if (gamepad.buttons[14].pressed || axisX < -0.7) {
//         console.log("D-pad left pressed");
//     }
//     // Check if D-pad Up pressed or Y-Axis
//     else if (gamepad.buttons[12].pressed || axisY < -0.7) {
//         console.log("D-pad up pressed");
//     }
//     // Check if D-pad Right pressed or X Axis > 0.7
//     else if (gamepad.buttons[15].pressed || axisX > 0.7) {
//         console.log("D-pad Right pressed");
//     }
//     // Check if D-pad Down pressed or Y Axis > 0.7
//     else if (gamepad.buttons[13].pressed || axisY > 0.7) {
//         console.log("D-pad Down pressed");
//     };
//     //#endregion


//     //#region + Button Pressed Check (Save)
//     if (gamepad.buttons[9].pressed) {
//         console.log("+ (Plus) button pressed");
//     }
//     //#endregion

//     //#region R-Stick Y Value Calculation (Description scroll)
//     var RStickYValue = gamepad.axes[3].toFixed(2);
//     //#endregion

//     //#region L and R button Pressed Checkd (Category Switching)
//     if (gamepad.buttons[4].pressed) {
//         console.log("L Button Pressed");
//     }

//     if (gamepad.buttons[5].pressed) {
//         console.log("R Button Pressed");
//     }
//     //#endregion
// }

function loadSVG() {
    $('.svg-container').each(function() {
        var $thisObj = $(this);
        $(this).load($thisObj.attr("ref"));
        $(this).addClass("is-appear");
    });
}

window.addEventListener("DOMContentLoaded", (e) => {
    window.addEventListener('NXFirstPaintEndAfterLoad', function() {
        setTimeout(loadSVG, 0);
    });

    // Listen to the gamepadconnected event
    window.addEventListener("gamepadconnected", function(e) {
        // Once a gamepad has connected, start an interval function that will run every 100ms to check for input
        setInterval(function() {
            var gpl = navigator.getGamepads();
            if (gpl[0] != null || gpl[0] != undefined) { checkGamepad(0, gpl[0]); }
        }, 150);
    });

    $(function() {
        if (!isNx) {
            setTimeout(loadSVG, 0)
        }
    })
});

function scroll(target, offset, activeContainer) {
    // Check to see if mod is completely in view
    var fully = checkInView(target, false, activeContainer) == undefined ? false : true;

    // If so, then just focus and dip
    if (fully) {
        target.focus();
    } else {
        // Remove focus from currently focused one
        $(".is-focused").focusout();
        // Stop any animation going on in the container
        activeContainer.stop();
        // Animate the mod container scrolling with a speed of 0 (fastest)
        activeContainer.animate({
            scrollTop: offset
        }, 0);
        // Focus on the previous mod
        target.focus();
    }
}

// yonked from here https://stackoverflow.com/questions/16308037/detect-when-elements-within-a-scrollable-div-are-out-of-view
function checkInView(elem, partial, activeContainer) {
    var container = $(activeContainer);
    var contHeight = container.height();
    var contTop = container.scrollTop();
    var contBottom = contTop + contHeight;

    var elemTop = $(elem).offset().top - container.offset().top;
    var elemBottom = elemTop + $(elem).height();

    var isTotal = (elemTop >= 0 && elemBottom <= contHeight);
    var isPart = ((elemTop < 0 && elemBottom > 0) || (elemTop > 0 && elemTop <= container.height())) && partial;

    return isTotal || isPart;
}