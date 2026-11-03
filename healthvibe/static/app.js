(function () {
  var el = document.getElementById("chart");
  if (!el || typeof echarts === "undefined") {
    return;
  }
  var chart = echarts.init(el);
  var color = getComputedStyle(document.body).color;
  chart.setOption({
    backgroundColor: "transparent",
    title: {
      text: "No data yet",
      left: "center",
      top: "middle",
      textStyle: {
        color: color,
        fontWeight: 500,
        fontSize: 16,
      },
    },
    series: [],
  });
  window.addEventListener("resize", function () {
    chart.resize();
  });
})();
