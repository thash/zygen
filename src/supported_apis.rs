// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct SupportedApi {
    pub name: String,     // e.g., "appengine"
    pub title: String,    // e.g., "App Engine Admin"
    pub category: String, // e.g., "Compute"
    pub aliases: Vec<String>,
    pub versions: Vec<String>,
}

impl SupportedApi {
    /// Returns the first version as the default
    pub fn default_version(&self) -> &str {
        self.versions
            .first()
            .expect("There should be at least one version")
    }
}

macro_rules! api {
    ($name:literal, $title:literal, $category:literal, [$($alias:literal),*], [$($version:literal),+]) => {
        SupportedApi {
            name: $name.to_string(),
            title: $title.to_string(),
            category: $category.to_string(),
            aliases: vec![$($alias.to_string()),*],
            versions: vec![$($version.to_string()),+],
        }
    };
}

/// List of APIs that zygen support (undocumented versions are excluded).
/// Categories are based on: https://cloud.google.com/terms/services, https://console.cloud.google.com/products, and console UI
#[rustfmt::skip]
static PRIMARY_SUPPORTED_APIS: LazyLock<Vec<SupportedApi>> = LazyLock::new(||
    vec![
        api!("accessapproval"         , "Access Approval"                               , "Identity & Access", ["access-approval"]             , ["v1"]),
        api!("accesscontextmanager"   , "Access Context Manager"                        , "Identity & Access", ["acm"]                         , ["v1"]),
        api!("aiplatform"             , "Vertex AI"                                     , "AI/ML"            , ["vertex", "ai"]                , ["v1beta1", "v1"]),
        api!("alloydb"                , "AlloyDB"                                       , "Databases"        , ["alloy"]                       , ["v1beta", "v1"]),
        api!("apigateway"             , "API Gateway"                                   , "Serverless"       , ["api-gateway"]                 , ["v1beta", "v1"]),
        api!("apigee"                 , "Apigee"                                        , "Integration"      , []                              , ["v1"]),
        api!("appengine"              , "App Engine Admin"                              , "Serverless"       , ["app"]                         , ["v1", "v1beta"]),
        api!("artifactregistry"       , "Artifact Registry"                             , "Developer"        , ["artifacts"]                   , ["v1"]),
        api!("assuredworkloads"       , "Assured Workloads"                             , "Security"         , ["assured-workloads"]           , ["v1", "v1beta1"]),
        api!("backupdr"               , "Google Cloud Backup and DR"                    , "Operations"       , ["backup-dr"]                   , ["v1"]),
        api!("baremetalsolution"      , "Bare Metal Solution"                           , "Compute"          , ["bms"]                         , ["v2"]),
        api!("batch"                  , "Batch"                                         , "Compute"          , []                              , ["v1"]),
        api!("bigquery"               , "BigQuery"                                      , "Analytics"        , ["bq"]                          , ["v2"]),
        api!("bigtableadmin"          , "Cloud Bigtable Admin"                          , "Databases"        , ["bigtable"]                    , ["v2"]),
        api!("binaryauthorization"    , "Binary Authorization"                          , "Security"         , ["binary-auth"]                 , ["v1", "v1beta1"]),
        api!("blockchainnodeengine"   , "Blockchain Node Engine"                        , "Compute"          , ["bne", "blockchain"]           , ["v1"]),
        api!("certificatemanager"     , "Certificate Manager"                           , "Security"         , ["certificate-manager", "cert"] , ["v1"]),
        api!("cloudasset"             , "Cloud Asset"                                   , "Management"       , ["asset"]                       , ["v1", "v1p1beta1", "v1p7beta1"]),
        api!("cloudbuild"             , "Cloud Build"                                   , "Developer"        , ["build"]                       , ["v1", "v2"]),
        api!("clouddeploy"            , "Cloud Deploy"                                  , "Developer"        , ["deploy"]                      , ["v1"]),
        api!("cloudfunctions"         , "Cloud Run functions"                           , "Serverless"       , ["functions", "func"]           , ["v2", "v2beta", "v2alpha", "v1"]), // formerly Cloud Functions
        api!("cloudidentity"          , "Cloud Identity"                                , "Identity & Access", ["identity"]                    , ["v1", "v1beta1"]),
        api!("cloudkms"               , "Cloud Key Management Service"                  , "Security"         , ["kms"]                         , ["v1"]),
        api!("cloudprofiler"          , "Cloud Profiler"                                , "Operations"       , ["profiler"]                    , ["v2"]),
        api!("cloudresourcemanager"   , "Cloud Resource Manager"                        , "Management"       , ["resource-manager", "resource"], ["v3", "v2", "v2beta1", "v1", "v1beta1"]),
        api!("cloudscheduler"         , "Cloud Scheduler"                               , "Integration"      , ["scheduler"]                   , ["v1", "v1beta1"]),
        api!("cloudshell"             , "Cloud Shell"                                   , "Management"       , ["shell"]                       , ["v1"]),
        api!("cloudtasks"             , "Cloud Tasks"                                   , "Integration"      , ["tasks"]                       , ["v2", "v2beta3"]),
        api!("cloudtrace"             , "Cloud Trace"                                   , "Operations"       , ["trace"]                       , ["v2", "v2beta1", "v1"]),
        api!("composer"               , "Cloud Composer"                                , "Analytics"        , []                              , ["v1beta1", "v1"]),
        api!("compute"                , "Compute Engine"                                , "Compute"          , ["gce"]                         , ["v1", "beta"]),
        api!("contactcenteraiplatform", "Conversational AI"                             , "AI/ML"            , ["conv-ai", "ccai"]             , ["v1alpha1"]), // formerly Contact Center AI (CCAI)
        api!("container"              , "Google Kubernetes Engine"                      , "Compute"          , ["gke"]                         , ["v1", "v1beta1"]),
        api!("datacatalog"            , "Google Cloud Data Catalog"                     , "Analytics"        , ["data-catalog"]                , ["v1", "v1beta1"]),
        api!("dataflow"               , "Dataflow"                                      , "Analytics"        , []                              , ["v1b3"]),
        api!("dataform"               , "Dataform"                                      , "Analytics"        , []                              , ["v1beta1"]),
        api!("datafusion"             , "Cloud Data Fusion"                             , "Analytics"        , ["data-fusion"]                 , ["v1beta1", "v1"]),
        api!("datamigration"          , "Database Migration Service"                    , "Migration"        , ["dms"]                         , ["v1", "v1beta1"]),
        api!("dataplex"               , "Cloud Dataplex"                                , "Analytics"        , []                              , ["v1"]),
        api!("dataproc"               , "Cloud Dataproc"                                , "Analytics"        , []                              , ["v1"]),
        api!("datastore"              , "Cloud Datastore"                               , "Databases"        , []                              , ["v1"]),
        api!("datastream"             , "Datastream"                                    , "Analytics"        , []                              , ["v1"]),
        api!("deploymentmanager"      , "Cloud Deployment Manager"                      , "Management"       , ["deployment-manager"]          , ["v2", "v2beta"]),
        api!("developerconnect"       , "Developer Connect"                             , "Developer"        , ["developer-connect"]           , ["v1"]),
        api!("dlp"                    , "Cloud Data Loss Prevention"                    , "Security"         , []                              , ["v2"]),
        api!("dns"                    , "Cloud DNS"                                     , "Networking"       , []                              , ["v1", "v1beta2"]),
        api!("documentai"             , "Cloud Document AI"                             , "AI/ML"            , ["doc-ai"]                      , ["v1", "v1beta3"]),
        api!("eventarc"               , "Eventarc"                                      , "Serverless"       , []                              , ["v1"]),
        api!("file"                   , "Cloud Filestore"                               , "Storage"          , []                              , ["v1", "v1beta1"]),
        api!("firestore"              , "Cloud Firestore"                               , "Databases"        , []                              , ["v1", "v1beta1", "v1beta2"]),
        api!("healthcare"             , "Cloud Healthcare"                              , "Analytics"        , []                              , ["v1", "v1beta1"]),
        api!("iam"                    , "Identity and Access Management"                , "Identity & Access", []                              , ["v1", "v2"]),
        api!("iap"                    , "Cloud Identity-Aware Proxy"                    , "Identity & Access", []                              , ["v1", "v1beta1"]),
        api!("ids"                    , "Cloud Intrusion Detection System"              , "Security"         , []                              , ["v1"]),
        api!("language"               , "Cloud Natural Language"                        , "AI/ML"            , []                              , ["v2", "v1", "v1beta2"]),
        api!("lifesciences"           , "Cloud Life Sciences"                           , "Analytics"        , []                              , ["v2beta"]), // formerly Google Genomics
        api!("logging"                , "Cloud Logging"                                 , "Operations"       , ["log"]                         , ["v2"]),
        api!("looker"                 , "Looker (Google Cloud core)"                    , "Analytics"        , []                              , ["v1"]),
        api!("managedidentities"      , "Managed Service for Microsoft Active Directory", "Identity & Access", ["managed-ad"]                  , ["v1", "v1beta1"]),
        api!("migrationcenter"        , "Migration Center"                              , "Migration"        , ["migration-center"]            , ["v1", "v1alpha1"]),
        api!("monitoring"             , "Cloud Monitoring"                              , "Operations"       , ["mon"]                         , ["v3", "v1"]),
        api!("networkconnectivity"    , "Network Connectivity Center"                   , "Networking"       , ["ncc"]                         , ["v1", "v1alpha1"]),
        api!("networkmanagement"      , "Network Intelligence Center"                   , "Networking"       , ["network-management"]          , ["v1", "v1beta1"]),
        api!("orgpolicy"              , "Organization Policy"                           , "Management"       , []                              , ["v2"]),
        api!("privateca"              , "Certificate Authority Service"                 , "Security"         , ["cas", "private-ca"]           , ["v1"]),
        api!("pubsub"                 , "Cloud Pub/Sub"                                 , "Analytics"        , []                              , ["v1"]),
        api!("recaptchaenterprise"    , "Google Cloud reCAPTCHA Enterprise"             , "Security"         , ["recaptcha"]                   , ["v1"]),
        api!("recommender"            , "Recommender"                                   , "Management"       , []                              , ["v1", "v1beta1"]),
        api!("redis"                  , "Memorystore for Redis"                         , "Databases"        , []                              , ["v1", "v1beta1"]),
        api!("run"                    , "Cloud Run Admin"                               , "Serverless"       , ["cloudrun"]                    , ["v2", "v1"]),
        api!("secretmanager"          , "Secret Manager"                                , "Security"         , ["secret"]                      , ["v1", "v1beta1"]),
        api!("securitycenter"         , "Security Command Center"                       , "Security"         , ["scc"]                         , ["v1", "v1beta2", "v1beta1"]),
        api!("servicedirectory"       , "Service Directory"                             , "Networking"       , ["service-directory"]           , ["v1", "v1beta1"]),
        api!("serviceusage"           , "Service Usage"                                 , "Management"       , ["service", "svc"]              , ["v1beta1", "v1"]),
        api!("spanner"                , "Cloud Spanner"                                 , "Databases"        , ["span"]                        , ["v1"]),
        api!("sqladmin"               , "Cloud SQL Admin"                               , "Databases"        , ["sql"]                         , ["v1beta4", "v1"]),
        api!("storage"                , "Cloud Storage"                                 , "Storage"          , ["gs", "gcs"]                   , ["v1"]),
        api!("storagetransfer"        , "Storage Transfer Service"                      , "Migration"        , ["storage-transfer"]            , ["v1"]),
        api!("trafficdirector"        , "Traffic Director (Cloud Service Mesh)"         , "Networking"       , ["traffic-director"]            , ["v2", "v3"]),
        api!("transcoder"             , "Transcoder"                                    , "Compute"          , []                              , ["v1"]),
        api!("translate"              , "Cloud Translation"                             , "AI/ML"            , []                              , ["v3", "v3beta1"]),
        api!("videointelligence"      , "Cloud Video Intelligence"                      , "AI/ML"            , ["video-intelligence"]          , ["v1", "v1p3beta1"]),
        api!("vision"                 , "Cloud Vision"                                  , "AI/ML"            , []                              , ["v1"]),
        api!("vmmigration"            , "Migrate to Virtual Machines (VM Migration)"    , "Migration"        , ["vm-migration"]                , ["v1"]),
        api!("vmwareengine"           , "Google Cloud VMware Engine (GCVE)"             , "Compute"          , ["gcve"]                        , ["v1"]),
        api!("webrisk"                , "Web Risk"                                      , "Security"         , []                              , ["v1"]),
        api!("websecurityscanner"     , "Web Security Scanner"                          , "Security"         , ["web-security-scanner"]        , ["v1", "v1beta"]),
        api!("workflows"              , "Workflows"                                     , "Serverless"       , []                              , ["v1", "v1beta"]),
        api!("workloadmanager"        , "Workload Manager"                              , "Compute"          , ["wlm"]                         , ["v1"]),
        api!("workstations"           , "Cloud Workstations"                            , "Developer"        , []                              , ["v1", "v1beta"]),
    ]
);

/// List of APIs that zygen support (undocumented versions are excluded), but
///   not explicitly mentioned in https://cloud.google.com/terms/services,
///   or a larger scope service is already included in in PRIMARY, or direct API access is uncommon.
#[rustfmt::skip]
static SECONDARY_SUPPORTED_APIS: LazyLock<Vec<SupportedApi>> = LazyLock::new(||
    vec![
        api!("advisorynotifications"    , "Advisory Notifications"                , "Security"         , ["advisory-notifications"]                 , ["v1"]),
        api!("analyticshub"             , "BigQuery Analytics Hub"                , "Analytics"        , ["analytics-hub"]                          , ["v1", "v1beta1"]),
        api!("apigeeregistry"           , "Apigee Registry"                       , "Integration"      , ["apigee-registry"]                        , ["v1"]),
        api!("apikeys"                  , "API Keys"                              , "Management"       , []                                         , ["v2"]),
        api!("apim"                     , "Apigee API Management (Observation)"   , "Integration"      , []                                         , ["v1alpha"]),
        api!("apphub"                   , "App Hub"                               , "Operations"       , []                                         , ["v1", "v1alpha"]),
        api!("beyondcorp"               , "Beyondcorp (Chrome Enterprise Premium)", "Security"         , []                                         , ["v1"]),
        api!("biglake"                  , "BigLake"                               , "Analytics"        , []                                         , ["v1"]),
        api!("bigqueryconnection"       , "BigQuery Connection"                   , "Analytics"        , ["bq-connection"]                          , ["v1", "v1beta1"]),
        api!("bigquerydatapolicy"       , "BigQuery Data Policy"                  , "Analytics"        , ["bq-policy"]                              , ["v1"]),
        api!("bigquerydatatransfer"     , "BigQuery Data Transfer Service"        , "Migration"        , ["bq-dts"]                                 , ["v1"]),
        api!("bigqueryreservation"      , "BigQuery Reservation"                  , "Analytics"        , ["bq-reservation"]                         , ["v1"]),
        api!("billingbudgets"           , "Cloud Billing Budget"                  , "Management"       , ["billing-budgets"]                        , ["v1", "v1beta1"]),
        api!("cloudbilling"             , "Cloud Billing"                         , "Management"       , ["billing"]                                , ["v1beta", "v1"]),
        api!("cloudchannel"             , "Cloud Channel"                         , "Management"       , []                                         , ["v1"]),
        api!("cloudcontrolspartner"     , "Cloud Controls Partner"                , "Management"       , []                                         , ["v1", "v1beta"]),
        api!("clouderrorreporting"      , "Error Reporting"                       , "Operations"       , ["error-reporting"]                        , ["v1beta1"]),
        api!("cloudsupport"             , "Google Cloud Support"                  , "Management"       , ["support"]                                , ["v2", "v2beta"]),
        api!("config"                   , "Infrastructure Manager"                , "Management"       , ["infra-manager"]                          , ["v1"]),
        api!("connectors"               , "Integration Connectors"                , "Integration"      , []                                         , ["v1"]),
        api!("contactcenterinsights"    , "Conversational Insights"               , "AI/ML"            , ["conv-insights", "ccai-insights"]         , ["v1"]), // formerly Contact Center AI Insights
        api!("containeranalysis"        , "Container Analysis"                    , "Security"         , ["container-analysis", "artifact-analysis"], ["v1", "v1beta1"]),
        api!("contentwarehouse"         , "Document AI Warehouse"                 , "AI/ML"            , ["doc-ai-warehouse"]                       , ["v1"]),
        api!("datalineage"              , "Data Lineage"                          , "Analytics"        , ["data-lineage"]                           , ["v1"]),
        api!("datapipelines"            , "Data pipelines"                        , "Analytics"        , ["data-pipelines"]                         , ["v1"]),
        api!("dialogflow"               , "Dialogflow"                            , "AI/ML"            , []                                         , ["v3", "v3beta1", "v2", "v2beta1"]),
        api!("discoveryengine"          , "Vertex AI Agent Builder"               , "AI/ML"            , ["discovery-engine", "agent-builder"]      , ["v1", "v1beta", "v1alpha"]),
        api!("domains"                  , "Cloud Domains"                         , "Networking"       , []                                         , ["v1", "v1beta1"]),
        api!("essentialcontacts"        , "Essential Contacts"                    , "Management"       , ["essential-contacts"]                     , ["v1"]),
        api!("gkebackup"                , "Backup for GKE"                        , "Storage"          , ["gke-backup"]                             , ["v1"]),
        api!("gkehub"                   , "GKE Hub (Fleet)"                       , "Compute"          , ["gke-hub", "fleet"]                       , ["v2", "v1beta1", "v2beta", "v2alpha", "v1", "v1beta", "v1alpha"]),
        api!("gkeonprem"                , "Google Distributed Cloud (GDC) Virtual", "Compute"          , ["gke-onprem"]                             , ["v1"]),
        api!("iamcredentials"           , "IAM Service Account Credentials"       , "Identity & Access", ["iam-credentials"]                        , ["v1"]),
        api!("identitytoolkit"          , "Identity Toolkit"                      , "Identity & Access", ["identity-toolkit"]                       , ["v2", "v1"]),
        api!("integrations"             , "Application Integration"               , "Integration"      , []                                         , ["v1"]),
        api!("jobs"                     , "Cloud Talent Solution"                 , "AI/ML"            , ["talent-solution"]                        , ["v3", "v3p1beta1"]),
        api!("kmsinventory"             , "KMS Inventory"                         , "Security"         , ["kms-inventory"]                          , ["v1"]),
        api!("memcache"                 , "Memorystore for Memcached"             , "Databases"        , []                                         , ["v1", "v1beta2"]),
        api!("metastore"                , "Dataproc Metastore"                    , "Analytics"        , ["dataproc-metastore"]                     , ["v1", "v1beta", "v1alpha"]),
        api!("networksecurity"          , "Network Security (Service Mesh)"       , "Networking"       , ["network-security"]                       , ["v1beta1"]),
        api!("networkservices"          , "Network Services (Service Mesh)"       , "Networking"       , ["network-services"]                       , ["v1", "v1beta1"]),
        api!("notebooks"                , "Vertex AI Workbench Notebooks"         , "AI/ML"            , []                                         , ["v1", "v2"]),
        api!("ondemandscanning"         , "On-Demand Scanning"                    , "Security"         , ["ondemand-scanning"]                      , ["v1"]),
        api!("oracledatabase"           , "Oracle Database@Google Cloud"          , "Databases"        , ["oracle-database"]                        , ["v1"]),
        api!("osconfig"                 , "OS Config"                             , "Management"       , ["os-config"]                              , ["v1", "v1beta", "v1alpha", "v2beta"]),
        api!("oslogin"                  , "Cloud OS Login"                        , "Security"         , ["os-login"]                               , ["v1", "v1beta", "v1alpha"]),
        api!("policysimulator"          , "Policy Simulator"                      , "Security"         , ["policy-simulator"]                       , ["v1", "v1beta"]),
        api!("policytroubleshooter"     , "Policy Troubleshooter"                 , "Management"       , ["policy-troubleshooter"]                  , ["v1"]),
        api!("publicca"                 , "Public Certificate Authority"          , "Security"         , ["public-ca"]                              , ["v1"]),
        api!("pubsublite"               , "Pub/Sub Lite"                          , "Analytics"        , ["pubsub-lite"]                            , ["v1"]),
        api!("rapidmigrationassessment" , "Rapid Migration Assessment"            , "Migration"        , ["ramp"]                                   , ["v1"]),
        api!("recommendationengine"     , "Recommendations AI"                    , "AI/ML"            , ["recommendation-engine"]                  , ["v1beta1"]),
        api!("resourcesettings"         , "Resource Settings"                     , "Management"       , ["resource-settings"]                      , ["v1"]),
        api!("retail"                   , "Vertex AI Search for Retail"           , "AI/ML"            , []                                         , ["v2", "v2beta", "v2alpha"]),
        api!("runtimeconfig"            , "Cloud Runtime Configuration"           , "Management"       , ["runtime-config"]                         , ["v1beta1"]),
        api!("serviceconsumermanagement", "Service Consumer Management"           , "Management"       , ["service-consumer-management"]            , ["v1", "v1beta1"]),
        api!("servicecontrol"           , "Service Control"                       , "Management"       , ["service-control"]                        , ["v2", "v1"]),
        api!("servicemanagement"        , "Service Management"                    , "Management"       , ["service-management"]                     , ["v1"]),
        api!("servicenetworking"        , "Service Networking"                    , "Networking"       , ["service-networking"]                     , ["v1"]),
        api!("speech"                   , "Cloud Speech-to-Text"                  , "AI/ML"            , ["speech-to-text"]                         , ["v1", "v1p1beta1"]),
        api!("sts"                      , "Security Token Service"                , "Security"         , []                                         , ["v1"]),
        api!("texttospeech"             , "Cloud Text-to-Speech"                  , "AI/ML"            , ["text-to-speech"]                         , ["v1", "v1beta1"]),
        api!("tpu"                      , "Cloud TPU"                             , "Compute"          , []                                         , ["v2", "v2alpha1", "v1", "v1alpha1"]),
        api!("vpcaccess"                , "Serverless VPC Access"                 , "Networking"       , ["vpc-access"]                             , ["v1", "v1beta1"]),
        api!("workflowexecutions"       , "Workflow Executions"                   , "Serverless"       , ["workflow-executions"]                    , ["v1", "v1beta"]),
    ]
);

/// List of APIs that are not included in the response of the Discovery API (`discovery::DISCOVERY_URL`).
/// zygen will download these API definitions when needed through `core::lazy_prep_api_file``.
#[rustfmt::skip]
static STANDALONE_DISCOVERY_APIS: LazyLock<Vec<SupportedApi>> = LazyLock::new(||
    vec![
        api!("generativelanguage", "Gemini", "AI/ML", ["gemini"], ["v1beta"]),
    ]
);

/// Returns a list of supported APIs.
/// If `all_apis` is true, it includes all APIs, otherwise only the primary and the standalone APIs.
pub fn supported_apis(all_apis: bool) -> Vec<SupportedApi> {
    let mut apis = PRIMARY_SUPPORTED_APIS.to_vec();
    match all_apis {
        true => {
            apis.extend(SECONDARY_SUPPORTED_APIS.iter().cloned());
            apis.extend(STANDALONE_DISCOVERY_APIS.iter().cloned());
        }
        false => apis.extend(STANDALONE_DISCOVERY_APIS.iter().cloned()),
    }
    apis
}

/// Returns a list of standalone APIs that are not included in the response of the Discovery API.
pub fn standalone_apis() -> Vec<SupportedApi> {
    STANDALONE_DISCOVERY_APIS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_overlaps() {
        let all_services = supported_apis(true);

        // Map to track all names and aliases to their corresponding service names
        let mut name_to_service = std::collections::HashMap::new();

        for service in all_services.iter() {
            // Check for duplicate service names
            if let Some(existing_service) =
                name_to_service.insert(service.name.clone(), service.name.clone())
            {
                if existing_service != service.name {
                    panic!(
                        "Service name '{}' conflicts with another service name '{}'",
                        service.name, existing_service
                    );
                }
            }

            // Check for overlapping aliases
            for alias in service.aliases.iter() {
                if let Some(existing_service) =
                    name_to_service.insert(alias.clone(), service.name.clone())
                {
                    if existing_service != service.name {
                        panic!(
                            "Alias '{}' for service '{}' conflicts with alias or name of another service '{}'",
                            alias, service.name, existing_service
                        );
                    }
                }
            }
        }
    }
}
