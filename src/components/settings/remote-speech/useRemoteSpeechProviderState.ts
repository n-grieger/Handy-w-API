import { useCallback, useMemo } from "react";
import { useSettings } from "../../../hooks/useSettings";
import type { DropdownOption } from "../../ui/Dropdown";
import type { ModelOption } from "../PostProcessingSettingsApi/types";

type RemoteSpeechProviderState = {
  providerOptions: DropdownOption[];
  selectedProviderId: string;
  selectedProvider: { id: string; label: string; base_url: string } | undefined;
  isCustomProvider: boolean;
  baseUrl: string;
  handleBaseUrlChange: (value: string) => void;
  isBaseUrlUpdating: boolean;
  apiKey: string;
  handleApiKeyChange: (value: string) => void;
  isApiKeyUpdating: boolean;
  model: string;
  handleModelChange: (value: string) => void;
  modelOptions: ModelOption[];
  isModelUpdating: boolean;
  isFetchingModels: boolean;
  handleProviderSelect: (providerId: string) => void;
  handleModelSelect: (value: string) => void;
  handleModelCreate: (value: string) => void;
  handleRefreshModels: () => void;
  /** Whether remote transcription is actually enabled (provider + URL + model). */
  isRemoteEnabled: boolean;
};

export const useRemoteSpeechProviderState = (): RemoteSpeechProviderState => {
  const {
    settings,
    isUpdating,
    setRemoteSpeechProvider,
    updateRemoteSpeechBaseUrl,
    updateRemoteSpeechApiKey,
    updateRemoteSpeechModel,
    fetchRemoteSpeechModels,
    remoteSpeechModelOptions,
  } = useSettings();

  const providers = settings?.remote_speech_providers || [];

  const selectedProviderId = useMemo(
    () => settings?.remote_speech_provider_id || providers[0]?.id || "none",
    [providers, settings?.remote_speech_provider_id],
  );

  const selectedProvider = useMemo(
    () => providers.find((p) => p.id === selectedProviderId) || providers[0],
    [providers, selectedProviderId],
  );

  const isCustomProvider = selectedProvider?.id === "custom";
  const baseUrl = selectedProvider?.base_url ?? "";
  const apiKey = settings?.remote_speech_api_keys?.[selectedProviderId] ?? "";
  const model = settings?.remote_speech_models?.[selectedProviderId] ?? "";

  const providerOptions = useMemo<DropdownOption[]>(
    () => providers.map((p) => ({ value: p.id, label: p.label })),
    [providers],
  );

  const isRemoteEnabled =
    isCustomProvider && baseUrl.trim() !== "" && model.trim() !== "";

  const handleProviderSelect = useCallback(
    async (providerId: string) => {
      if (providerId === selectedProviderId) return;
      await setRemoteSpeechProvider(providerId);
      // Auto-fetch models when switching to the custom endpoint with a URL set.
      if (providerId === "custom") {
        const provider = providers.find((p) => p.id === providerId);
        if ((provider?.base_url ?? "").trim() !== "") {
          void fetchRemoteSpeechModels(providerId);
        }
      }
    },
    [selectedProviderId, setRemoteSpeechProvider, fetchRemoteSpeechModels, providers],
  );

  const handleBaseUrlChange = useCallback(
    (value: string) => {
      if (!isCustomProvider) return;
      const trimmed = value.trim();
      if (trimmed && trimmed !== baseUrl) {
        void updateRemoteSpeechBaseUrl(selectedProviderId, trimmed);
      }
    },
    [isCustomProvider, baseUrl, updateRemoteSpeechBaseUrl, selectedProviderId],
  );

  const handleApiKeyChange = useCallback(
    (value: string) => {
      const trimmed = value.trim();
      if (trimmed !== apiKey) {
        void updateRemoteSpeechApiKey(selectedProviderId, trimmed);
      }
    },
    [apiKey, updateRemoteSpeechApiKey, selectedProviderId],
  );

  const handleModelChange = useCallback(
    (value: string) => {
      const trimmed = value.trim();
      if (trimmed !== model) {
        void updateRemoteSpeechModel(selectedProviderId, trimmed);
      }
    },
    [model, updateRemoteSpeechModel, selectedProviderId],
  );

  const handleModelSelect = useCallback(
    (value: string) => {
      void updateRemoteSpeechModel(selectedProviderId, value.trim());
    },
    [updateRemoteSpeechModel, selectedProviderId],
  );

  const handleModelCreate = useCallback(
    (value: string) => {
      void updateRemoteSpeechModel(selectedProviderId, value);
    },
    [updateRemoteSpeechModel, selectedProviderId],
  );

  const handleRefreshModels = useCallback(() => {
    if (!isCustomProvider) return;
    void fetchRemoteSpeechModels(selectedProviderId);
  }, [isCustomProvider, fetchRemoteSpeechModels, selectedProviderId]);

  const availableModelsRaw = remoteSpeechModelOptions[selectedProviderId] || [];
  const modelOptions = useMemo<ModelOption[]>(() => {
    const seen = new Set<string>();
    const options: ModelOption[] = [];
    const upsert = (value?: string | null) => {
      const trimmed = value?.trim();
      if (!trimmed || seen.has(trimmed)) return;
      seen.add(trimmed);
      options.push({ value: trimmed, label: trimmed });
    };
    for (const candidate of availableModelsRaw) upsert(candidate);
    upsert(model);
    return options;
  }, [availableModelsRaw, model]);

  const isBaseUrlUpdating = isUpdating(`remote_speech_base_url:${selectedProviderId}`);
  const isApiKeyUpdating = isUpdating(`remote_speech_api_key:${selectedProviderId}`);
  const isModelUpdating = isUpdating(`remote_speech_model:${selectedProviderId}`);
  const isFetchingModels = isUpdating(`remote_speech_models_fetch:${selectedProviderId}`);

  return {
    providerOptions,
    selectedProviderId,
    selectedProvider,
    isCustomProvider,
    baseUrl,
    handleBaseUrlChange,
    isBaseUrlUpdating,
    apiKey,
    handleApiKeyChange,
    isApiKeyUpdating,
    model,
    handleModelChange,
    modelOptions,
    isModelUpdating,
    isFetchingModels,
    handleProviderSelect,
    handleModelSelect,
    handleModelCreate,
    handleRefreshModels,
    isRemoteEnabled,
  };
};
