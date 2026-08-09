<script setup lang="ts">
const disks = [
  {
    disk_id: "fixture-disk",
    runtime_status: "READY",
    export_job_status: "PENDING",
    current_object: "等待导出对象",
  },
];
</script>

<template>
  <main class="dashboard">
    <section class="toolbar">
      <div>
        <p class="eyebrow">Edge</p>
        <h1>边缘端 Dashboard</h1>
      </div>
      <span class="badge">HTTP + WebSocket</span>
    </section>

    <section class="grid">
      <article v-for="disk in disks" :key="disk.disk_id" class="card">
        <h2>{{ disk.disk_id }}</h2>
        <dl>
          <div>
            <dt>运行态</dt>
            <dd>{{ disk.runtime_status }}</dd>
          </div>
          <div>
            <dt>导出任务</dt>
            <dd>{{ disk.export_job_status }}</dd>
          </div>
        </dl>
        <p>{{ disk.current_object }}</p>
      </article>
    </section>
  </main>
</template>
