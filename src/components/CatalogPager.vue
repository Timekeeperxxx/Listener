<script setup lang="ts">
import { LoaderCircle } from '@lucide/vue'
import type { LoadState } from '../types'
defineProps<{ state: LoadState; error: string; hasMore: boolean; loaded: number; total: number }>()
defineEmits<{ load: [] }>()
</script>

<template>
  <div v-if="state === 'loading' || state === 'error' || hasMore" class="catalog-pager" aria-live="polite">
    <p v-if="error" class="catalog-error" role="alert">{{ error }}</p>
    <span v-if="hasMore && loaded">{{ loaded }} / {{ total }}</span>
    <button class="button secondary" :disabled="state === 'loading'" @click="$emit('load')"><LoaderCircle v-if="state === 'loading'" :size="15" class="spinning" />{{ state === 'loading' ? '正在加载' : error ? '重试' : '加载更多' }}</button>
  </div>
</template>
