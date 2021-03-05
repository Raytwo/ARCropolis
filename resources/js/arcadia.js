var modsContainer; // Used to hold a reference to the mods container that can be used anywhere 
var mods; // Used to hold the array of mods loaded

// To add more categories, just insert a new item
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
var selectedCategoryIndex = 0;


var currentDescHeight = 0; // Used for the current position of the description (modified by the R-Stick Y Value).
var currentActiveDescription // For reference to the current active description.
var activeDescHeight = 0; // The height for the current active description so it can't be scrolled out of bounds.

// Both used to make sure that players can't rapidly switch between categories
var LButtonHeld = [false, false, false, false];
var RButtonHeld = [false, false, false, false];

function toggleMod(e) {
    // Toggle the checkmark (disabled -> enabled and vice versa)
    document.getElementById(e.replace("btn-mods-", "img-")).classList.toggle("hidden");

    // :)
    window.navigator.vibrate([0, 50, 0]);
    
    // Remove the hidden class on the Save button
    if (document.getElementById("link-save").classList.contains("hidden")) {
        document.getElementById("link-save").classList.remove("hidden");
        document.getElementById("link-save").classList.add("show");
    }
};

function submitMods() {
    // Animate the Save button
    document.getElementById("link-save").classList.add("is-selected");

    // Wait for 700ms before running the following code (to let the Save Button animation play out)
    setTimeout(function (e) {
        // Create a new array that will be sent back to the Rust code
        var result = "";
        try {
            // Select all mods
            mods = document.querySelectorAll("#holder>button");
            // Create a i variable that's going to be used for ID
            var i = 0;
            // Loop through the selected mods and add them to the result          
            result += `is_disabled=[`;
            [].forEach.call(mods, function (a) {
                result += `${$(`#${a.id.replace("btn-mods-", "img-")}`).hasClass("hidden")}, `;
            });
            result += `]`;

            // Redirect back to localhost with the resultsArr converted to a string
            window.location.href = "http://localhost/" + result;
        }
        catch (throw_error) {
            // If there's an error, then display it to the user so that they can report back
            document.write(`Error! Please report the following to Coolsonickirby#4030 on discord (or ray if he wants to deal with javascript ig)!<br><br>${JSON.stringify(throw_error.message)}`);
        }
    }, 700);
}

// yoinked from here https://stackoverflow.com/questions/143815/determine-if-an-html-elements-content-overflows
function checkOverflow(el)
{
   var curOverflow = el.style.overflow;

   if ( !curOverflow || curOverflow === "visible" )
      el.style.overflow = "hidden";

   var isOverflowing = el.clientWidth < el.scrollWidth 
      || el.clientHeight < el.scrollHeight;

   el.style.overflow = curOverflow;

   return isOverflowing;
}


function updateCategory() {
    // Hide the R-Stick icon in-case user was on a Item with a long description
    document.getElementById("r-stick-desc-icon").style.visibility = "hidden";

    // Hide each mod description
    $('.l-main-content').each(function () {
        $(this).addClass("is-hidden");
    });

    // Update the current category
    document.getElementById("current_category").innerHTML = categories[selectedCategoryIndex];

    // Loop through each mod and display it if the selected category exists in the classList
    [].forEach.call(mods, function (mod) {
        if (mod.classList.contains(categories[selectedCategoryIndex])) {
            mod.style.display = "inline-flex";
        } else {
            mod.style.display = "none";
        }
    });

    // Focus on the first non-hidden mod item (if none, then nothing gets focused on)
    $('#holder button').each(function () {
        if ($(this).css('display') != 'none') {
            $(this).focus();
            return false;
        }
    });
}

function updateCurrentDesc() {
    // Reset current description height
    currentDescHeight = 0;

    // Loop through all description stuff and look for the one without the hidden class.
    $('.l-main-content').each(function () {
        if (!$(this).hasClass("is-hidden")) {
            // Once found, assign the description element to the global active description variable for use later
            currentActiveDescription = $(this).find(".l-description").eq(0);
            // Subtract 146 from the description scroll height to match the paragarph overflow
            activeDescHeight = currentActiveDescription[0].scrollHeight - 146;
            // Stop the loop
            return false;
        }
    });

    // Check to see if overflow occured and if so, enable the R-Stick Icon
    if(checkOverflow(currentActiveDescription[0])){
        document.getElementById("r-stick-desc-icon").style.visibility = "visible";
    }else{
        document.getElementById("r-stick-desc-icon").style.visibility = "hidden";
    }
}

// Check the gamepad input for saving, switching categories, and scrolling the description
function checkGamepad(index, gamepad) {
    //#region + Button Pressed Check (Save)
    if (gamepad.buttons[9].pressed) {
        if (!document.getElementById("link-save").classList.contains("hidden")) {
            document.getElementById("link-save").click();
        }
    }
    //#endregion

    //#region R-Stick Y Value Calculation (Description scroll)
    var RStickYValue = gamepad.axes[3].toFixed(2);

    RStickYValue = (((RStickYValue - 0) * (20 - 0)) / (1 - 0)) + 0;
    currentDescHeight += RStickYValue;

    if (currentDescHeight < 0) {
        currentDescHeight = 0;
    }
    else if (currentDescHeight > activeDescHeight) {
        currentDescHeight = activeDescHeight;
    }

    currentActiveDescription.scrollTop(currentDescHeight);
    //#endregion

    //#region L and R button Pressed Checkd (Category Switching)
    if (gamepad.buttons[4].pressed) {
        if (!LButtonHeld[index]) {
            selectedCategoryIndex = selectedCategoryIndex == 0 ? categories.length - 1 : selectedCategoryIndex - 1;
            updateCategory();
            LButtonHeld[index] = true;
        }
    } else {
        LButtonHeld[index] = false;
    };

    if (gamepad.buttons[5].pressed) {
        if (!RButtonHeld[index]) {
            selectedCategoryIndex = selectedCategoryIndex == categories.length - 1 ? 0 : selectedCategoryIndex + 1;
            updateCategory();
            RButtonHeld[index] = true;
        }
    } else {
        RButtonHeld[index] = false;
    };
    //#endregion
};


// yonked from here https://stackoverflow.com/questions/16308037/detect-when-elements-within-a-scrollable-div-are-out-of-view
function checkInView(elem, partial) {
    var container = modsContainer;
    var contHeight = container.height();
    var contTop = container.scrollTop();
    var contBottom = contTop + contHeight;

    var elemTop = $(elem).offset().top - container.offset().top;
    var elemBottom = elemTop + $(elem).height();

    var isTotal = (elemTop >= 0 && elemBottom <= contHeight);
    var isPart = ((elemTop < 0 && elemBottom > 0) || (elemTop > 0 && elemTop <= container.height())) && partial;

    return isTotal || isPart;
}


window.onload = function () {
    // Select the mod container
    modsContainer = $("#left-stick-home");
    
    // Select all mods
    mods = document.querySelectorAll("#holder>button");

    // Replace the icon in the submit button with a + icon
    document.getElementById("submit_icon").innerHTML = "&#xe0f1";

    // Listen to the keydown event and prevent the default
    window.addEventListener('keydown', function (e) {
        e.preventDefault();

        switch (event.keyCode) {
            case 32: // A Button Clicked
                $(".is-focused").click();
            case 37: // Left Dpad / Analog
                // Nothing to do for them currently
                break;
            case 38: // Up Dpad / Analog
                // Get the mod above the current focused one
                var target = $(".is-focused").prev();

                while(target.length > 0 && target.is(':hidden')){
                    target = target.prev();
                }

                // If that doesn't exist, then dip
                if (target.length <= 0) {
                    return;
                }

                // Check to see if mod is completely in view
                var fully = checkInView(target) == undefined ? false : true;

                // If so, then just focus and dip
                if (fully) {
                    target.focus();
                    return;
                } else {
                    // Remove focus from currently focused one
                    $(".is-focused").focusout();
                    // Stop any animation going on in the container
                    modsContainer.stop();
                    // Animate the mod container scrolling with a speed of 0 (fastest)
                    modsContainer.animate({
                        scrollTop: modsContainer.scrollTop() + target.position().top - 50
                    }, 0);
                    // Focus on the previous mod
                    target.focus();
                }
                break;
            case 39: // Right Dpad / Analog
                // Nothing to do for them currently
                break;
            case 40: // Down Dpad / Analog
                // Get the next mod that will be focused on
                var target = $(".is-focused").next();

                while(target.length > 0 && target.is(':hidden')){
                    target = target.next();
                }
                
                // If there is none after that, then just return
                if (target.length <= 0) {
                    return;
                }

                // See if the mod item is fully in view
                var fully = checkInView(target) == undefined ? false : true;

                // If it is completely visible, then just change focus and dip
                if (fully) {
                    target.focus();
                    return;
                } else {
                    // Remove the focus from the currently selected one
                    $(".is-focused").focusout();
                    // Stop any previous animation
                    modsContainer.stop();
                    // Animate the mod container scrolling with a speed of 0 (fastest)
                    modsContainer.animate({
                        scrollTop: (modsContainer.scrollTop()) + (target.height() * 2)
                    }, 0);
                    // Focus on the next mod
                    target.focus();
                }
                break;
        };
    });

    // Loop through each mod and resize the text to fit
    [].forEach.call(mods, function (i) {
        $(".mod-name", i).first().textfill({
            explicitWidth: 508,
            explicitHeight: 40,
            maxFontPixels: 23,
            changeLineHeight: 0.2
        });
    });

    // Listen to the gamepadconnected event
    window.addEventListener("gamepadconnected", function (e) {
        // Once a gamepad has connected, start an interval function that will run every 100ms to check for input
        setInterval(function () {
            var gpl = navigator.getGamepads();
            if (gpl.length > 0) {
                for (var i = 0; i < gpl.length; i++) {
                    checkGamepad(i, gpl[i]);
                }
            }
        }, 100);
     });
}