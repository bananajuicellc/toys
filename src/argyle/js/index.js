const canvas = document.getElementById('myCanvas');
const scale = 10;

function handleDrawButton(event) {
  // Prevent form submission from reloading page.
  event.preventDefault();
  
  const fMainColor = document.getElementById("f-main-color");
  const fSecondaryColor = document.getElementById("f-secondary-color");
  const fAccentColor = document.getElementById("f-accent-color");
  const mainColor = fMainColor.value;
  const secondaryColor = fSecondaryColor.value;
  const accentColor = fAccentColor.value;
  
  const fColorsMode = document.getElementById("f-colors-mode");
  const colorsMode = fColorsMode.value;
  
  const fVerticalRadius = document.getElementById("f-vertical-radius");
  const fHorizontalRadius = document.getElementById("f-horizontal-radius");
  const vRadius = parseInt(fVerticalRadius.value, 10);
  const hRadius = parseInt(fHorizontalRadius.value, 10);
  
  const fVerticalRepetitions = document.getElementById("f-vertical-repetitions");
  const fHorizontalRepetitions = document.getElementById("f-horizontal-repetitions");
  const vRepeat = parseInt(fVerticalRepetitions.value, 10);
  const hRepeat = parseInt(fHorizontalRepetitions.value, 10);
  
  const width = 2 * hRadius;
  const height = 2 * vRadius;
  canvas.width = width * hRepeat * scale;
  canvas.height = height * vRepeat * scale;
  
  const ctx = canvas.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  ctx.scale(10, 10); 
  
  pattern = renderToCanvas(
    width,
    height,
    buffer => renderArgyle(buffer, hRadius, vRadius, mainColor, secondaryColor, accentColor, colorsMode)
  );
  for (let rx = 0; rx < hRepeat; rx++) {
    for (let ry = 0; ry < vRepeat; ry++) {
        ctx.drawImage(
          pattern,
          rx * width,
          ry * height,
        );
    }
  }
}

function renderArgyle(
  buffer,
  hRadius,
  vRadius,
  primaryColor,
  secondaryColor,
  accentColor,
  colorsMode,
) {
  const width = 2 * hRadius;
  const height = 2 * vRadius;
  buffer.fillStyle = primaryColor;
  buffer.fillRect(0, 0, width, height);
  
  buffer.fillStyle = secondaryColor;
  renderDiamond(buffer, hRadius, vRadius);
  
  const centerWidth = Math.ceil(width / height);
  const centerHalfWidth = Math.floor(centerWidth / 2);
  const centerHeight = Math.ceil(height / width);
  const centerHalfHeight = Math.floor(centerHeight / 2);
  const centerX0 = hRadius - centerHalfWidth;
  const centerY0 = vRadius - centerHalfHeight;
  const centerX1 = hRadius + centerHalfWidth;
  const centerY1 = vRadius + centerHalfHeight;
  
  // TODO: support two color mode as well.
  if (colorsMode === "THREE_COLORS") {
    buffer.fillStyle = accentColor;
    // Split line so it always goes to the center.
    renderLine(
      buffer,
      0,
      0,
      centerX0 - 1,
      centerY0 - 1,
    );
    renderLine(
      buffer,
      centerX1 + 1,
      centerY1 - 1,
      width - 1,
      1,
    );
    buffer.fillRect(
      centerX0,
      centerY0,
      centerWidth,
      centerHeight,
    );
    renderLine(
      buffer,
      1,
      height - 1,
      centerX0 - 1,
      centerY1 + 1,
    );
    renderLine(
      buffer,
      centerX1 + 1,
      centerY1 + 1,
      width - 1,
      height - 1,
    );
  } else {
    
  }
}

// Draw full diamond background, top to bottom.
function renderDiamond(buffer, hRadius, vRadius) {
  const width = 2 * hRadius;
  const height = 2 * vRadius;
  for (let y = 0; y < height; y++) {
    const lineWidth = Math.min(
      1
      + Math.floor(  // Only max width at midpoint.
        width
        * (
          (y <= vRadius)
          ? (y / vRadius)
          : ((height - y) / vRadius)
        )
      ),
      width,
    );
    const nearestX = Math.ceil((width - lineWidth) / 2);
    buffer.fillRect(
      nearestX,
      y,
      lineWidth,
      1
    )
  }
}

// "pixel perfect" line interpolation.
// https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm
function renderLine(buffer, x0, y0, x1, y1) {
  console.log(`renderLine(${x0}, ${y0}, ${x1}, ${y1}`);
  const dx = Math.abs(x1 - x0);
  const slopeX = x0 < x1 ? 1 : -1;
  const dy = -1 * Math.abs(y1 - y0);
  const slopeY = y0 < y1 ? 1 : -1;
  // Technically the error should be scaled
  // by 1/2 to measure the error from
  // the center of pixels, but we scale by 2 to keep integer values.
  let error = slopeX + slopeY;
  for (let pixels = 0; pixels < 2 + (dx - dy); pixels++) {
    buffer.fillRect(x0, y0, 1, 1);
    let doubleError = 2 * error;
    if (doubleError >= dy) {
      if (x0 === x1) {
        break;
      }
      // Since dy is negative, this decreases the error.
      // Since we moved to the next x,
      // the accumulated error should
      // reset a bit.
      error = error + dy;
      x0 = x0 + slopeX;
    }
    if (doubleError <= dx) {
      if (y0 === y1) {
        break;
      }
      // Since dx is positive, this increases the error.
      // Since we moved to the next y,
      // the accumulated error increases.
      error = error + dx;
      y0 = y0 + slopeY;
    }
  }
  buffer.fillRect(x1, y1, 1, 1);
}

const drawButton = document.getElementById("f-draw");

drawButton.addEventListener("click", handleDrawButton);


// https://stackoverflow.com/a/8648229/101923
function renderToCanvas(width, height, renderFunction) {
    var buffer = document.createElement('canvas');
    buffer.width = width;
    buffer.height = height;
    renderFunction(buffer.getContext('2d'));
    return buffer;
};
