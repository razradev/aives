<script>
  let { flip } = $props();
</script>

<svg
  class="relative w-full h-full"
  viewBox="0 0 36 160"
  preserveAspectRatio="none"
  id="border"
>
  <rect
    width={flip ? "28" : "4"}
    height="160"
    x="0"
    y="0"
    fill="black"
    mask="url(#hole)"
  />
  <rect
    width={flip ? "4" : "32"}
    height="160"
    x={flip ? "32" : "8"}
    y="0"
    fill="black"
    mask="url(#hole)"
  />
  <ellipse cx={flip ? "-108" : "144"} cy="80" rx="128" ry="160" fill="white" />
</svg>
