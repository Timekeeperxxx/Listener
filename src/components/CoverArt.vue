<script setup lang="ts">
import { ref, watch } from 'vue'
import { AudioLines, Heart } from '@lucide/vue'
const props = withDefaults(defineProps<{ src?: string; alt?: string; liked?: boolean }>(), { src: '', alt: '' })
const failed = ref(false)
watch(() => props.src, () => { failed.value = false })
</script>

<template>
  <div class="cover-art" :class="{ 'cover-liked': liked }">
    <img v-if="src && !failed" :src="src" :alt="alt" loading="lazy" draggable="false" @error="failed = true" />
    <template v-else>
      <div class="cover-orb orb-one"></div><div class="cover-orb orb-two"></div><div class="cover-orb orb-three"></div>
      <Heart v-if="liked" class="cover-symbol" :stroke-width="1.2" />
      <AudioLines v-else class="cover-symbol" :stroke-width="1.2" />
    </template>
  </div>
</template>
