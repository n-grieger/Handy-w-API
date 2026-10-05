import React from "react";
import { useTranslation } from "react-i18next";
import { RefreshCcw, Cloud, CloudOff } from "lucide-react";
import { Dropdown, SettingContainer, SettingsGroup } from "@/components/ui";
import { ResetButton } from "../../ui/ResetButton";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { BaseUrlField } from "../PostProcessingSettingsApi/BaseUrlField";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { useRemoteSpeechProviderState } from "./useRemoteSpeechProviderState";

export const RemoteSpeechSettings: React.FC = () => {
  const { t } = useTranslation();
  const state = useRemoteSpeechProviderState();

  return (
    <SettingsGroup
      title={t("settings.remoteSpeech.title")}
      description={t("settings.remoteSpeech.description")}
    >
      <SettingContainer
        title={t("settings.remoteSpeech.provider.title")}
        description={t("settings.remoteSpeech.provider.description")}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={true}
      >
        <Dropdown
          options={state.providerOptions}
          selectedValue={state.selectedProviderId}
          onSelect={state.handleProviderSelect}
          className="flex-1 min-w-[240px]"
        />
      </SettingContainer>

      {state.isCustomProvider && (
        <>
          <SettingContainer
            title={t("settings.remoteSpeech.baseUrl.title")}
            description={t("settings.remoteSpeech.baseUrl.description")}
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <BaseUrlField
              value={state.baseUrl}
              onBlur={state.handleBaseUrlChange}
              placeholder={t("settings.remoteSpeech.baseUrl.placeholder")}
              disabled={state.isBaseUrlUpdating}
            />
          </SettingContainer>

          <SettingContainer
            title={t("settings.remoteSpeech.apiKey.title")}
            description={t("settings.remoteSpeech.apiKey.description")}
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <ApiKeyField
              value={state.apiKey}
              onBlur={state.handleApiKeyChange}
              placeholder={t("settings.remoteSpeech.apiKey.placeholder")}
              disabled={state.isApiKeyUpdating}
            />
          </SettingContainer>

          <SettingContainer
            title={t("settings.remoteSpeech.model.title")}
            description={t("settings.remoteSpeech.model.description")}
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ModelSelect
                value={state.model}
                options={state.modelOptions}
                disabled={state.isModelUpdating}
                isLoading={state.isFetchingModels}
                placeholder={t("settings.remoteSpeech.model.placeholder")}
                onSelect={state.handleModelSelect}
                onCreate={state.handleModelCreate}
                onBlur={() => {}}
                className="flex-1 min-w-[360px]"
              />
              <ResetButton
                onClick={state.handleRefreshModels}
                disabled={state.isFetchingModels}
                ariaLabel={t("settings.remoteSpeech.model.refresh")}
                className="flex h-10 w-10 items-center justify-center"
              >
                <RefreshCcw
                  className={`h-4 w-4 ${
                    state.isFetchingModels ? "animate-spin" : ""
                  }`}
                />
              </ResetButton>
            </div>
          </SettingContainer>
        </>
      )}

      <div className="flex items-center gap-2 px-4 py-2 text-sm">
        {state.isRemoteEnabled ? (
          <>
            <Cloud className="w-4 h-4 text-logo-primary shrink-0" />
            <span className="text-text/70">
              {t("settings.remoteSpeech.status.remote", {
                url: state.baseUrl,
                model: state.model,
              })}
            </span>
          </>
        ) : (
          <>
            <CloudOff className="w-4 h-4 text-text/50 shrink-0" />
            <span className="text-text/50">
              {t("settings.remoteSpeech.status.local")}
            </span>
          </>
        )}
      </div>
    </SettingsGroup>
  );
};
