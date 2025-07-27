<script>
  let { animating, userTyping } = $props();
</script>

<svg
  class="w-full h-full bg-size-[100%] {animating && 'animate-spin'}"
  viewBox="0 0 135.46666 135.46667"
  version="1.1"
  id="star"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  xmlns="http://www.w3.org/2000/svg"
  preserveAspectRatio="none"
>
  <defs id="defs2">
    <linearGradient id="linearGradient11935">
      <stop
        style="stop-color:{userTyping
          ? 'var(--color-g3)'
          : 'black'};stop-opacity:1;"
        offset="0"
        id="stop11931"
      />
      <stop
        style="stop-color:var(--color-{userTyping
          ? 'g4'
          : 'g4'});stop-opacity:1;"
        offset="0.5"
        id="stop13393"
      />
      <stop
        style="stop-color:var(--color-{userTyping
          ? 'g5'
          : 'g3'});stop-opacity:1;"
        offset="1"
        id="stop11933"
      />
    </linearGradient>
    <linearGradient
      xlink:href="#linearGradient11935"
      id="linearGradient11939"
      x1="-210.42564"
      y1="-2.8421709e-14"
      x2="210.42564"
      y2="-2.8421709e-14"
      gradientUnits="userSpaceOnUse"
    />
  </defs>
  <g id="layer1">
    <path
      style="fill:#ffffff;fill-opacity:0.5;stroke:url(#linearGradient11939);stroke-width:16.6235;stroke-linecap:square;stroke-linejoin:miter;stroke-miterlimit:100;stroke-dasharray:none;stroke-opacity:1;paint-order:normal;stop-color:#000000"
      id="path343"
      d="M 135.46666,0 35.195272,20.32 67.73333,117.31757 -1.7888513e-7,40.639999 -67.73333,117.31757 -35.195272,20.32 -135.46666,0 -35.195272,-20.32 -67.73333,-117.31757 1.7888513e-7,-40.639999 67.73333,-117.31757 35.195272,-20.32 Z"
      transform="matrix(0,0.38199282,-0.38199282,0,67.733332,67.733332)"
    />
  </g>
</svg>
