# zygen (zg): Zeroth Genotype CLI for Google Cloud APIs

zygen (zg) is a low-level CLI for Google Cloud APIs written in Rust, offering streamlined and minimal ways to browse ([zg ls](#zg-ls) / [zg desc](#zg-desc)) and execute ([zg ex](#zg-exec)) APIs. Leveraging [Discovery API](https://developers.google.com/discovery/v1/getting_started), zygen enables users to access fundamental raw API definitions — "genotype" — of Google Cloud services, without much abstraction. If you love using curl to call APIs (see: [How to Call Google APIs: REST Edition](https://googleapis.github.io/HowToREST) and [Authenticate for using REST](https://cloud.google.com/docs/authentication/rest)), zygen is for you.

Here's a quick demo video for zygen's capabilities:

[![zygen demo](https://thash.github.io/githubusercontent/zygen_capture.png)](https://thash.github.io/githubusercontent/zygen_demo.mp4)

With zygen, you can:
- List available Google Cloud services, resources, and methods quickly
  - Explore resource hierarchies in a tree-like structure for a better overview
- Get practical API method descriptions
  - Identify minimal parameters and request data to kickstart your API calls
  - Access links to official references for deeper insights
- Execute APIs from the command line like `curl`, but with more convenience
  - Automatically fill in "obvious" parameters in your context like Google Cloud Project Id and Region
  - Generate `curl` equivalents for easy reproducibility and sharing with others
- Integrate with other CLI tools like `jq` for advanced and standardized data manipulation


# Index

<!-- vscode-markdown-toc -->
1. [zygen (zg): Zeroth Genotype CLI for Google Cloud APIs](#zygen-zg-zeroth-genotype-cli-for-google-cloud-apis)
2. [Index](#index)
3. [Usage](#usage)
   1. [zg ls](#zg-ls)
      1. [List services of Google Cloud](#list-services-of-google-cloud)
      2. [List resources of a service](#list-resources-of-a-service)
      3. [List methods of a resource](#list-methods-of-a-resource)
         1. [Identify a resource uniquely](#identify-a-resource-uniquely)
   2. [zg desc](#zg-desc)
   3. [zg exec](#zg-exec)
      1. [Equivalent curl](#equivalent-curl)
   4. [zg update](#zg-update)
4. [Installation](#installation)
   1. [Homebrew (MacOS/Linux)](#homebrew-macoslinux)
   2. [Download binary](#download-binary)
   3. [Install from source](#install-from-source)
5. [Examples](#examples)
6. [Misc](#misc)
   1. [What zygen is not for](#what-zygen-is-not-for)

<!-- vscode-markdown-toc-config
	numbering=false
	autoSave=true
	/vscode-markdown-toc-config -->
<!-- /vscode-markdown-toc -->


# <a name='Usage'></a>Usage

## <a name='zgls'></a>zg ls

### <a name='ListservicesofGoogleCloud'></a>List services of Google Cloud

`zg ls` lists available Google Cloud services (APIs).

```
$ zg ls -a
...
aiplatform (vertex, ai)
alloydb (alloy)
...
cloudshell (shell)
cloudtasks (tasks)
cloudtrace (trace)
composer
compute (gce)
contactcenteraiplatform (conv-ai, ccai)
container (gke)
...
redis
run (cloudrun)
secretmanager (secret)
securitycenter (scc)
servicedirectory (service-directory)
serviceusage (service, svc)
spanner (span)
sqladmin (sql)
...
```

`--aliases (-a)` shows aliases of services. More options like `--long (-l)`, `--reverse (-r)`, `--category (-c)`, and `--sort (-S)` are available to enrich the output.

```
$ zg ls -ac --sort category --reverse
[Storage] Cloud Filestore - file
[Storage] Cloud Storage - storage (gs, gcs)
[Serverless] API Gateway - apigateway (api-gateway)
[Serverless] App Engine Admin - appengine (app)
[Serverless] Cloud Run functions - cloudfunctions (functions, func)
[Serverless] Eventarc - eventarc
[Serverless] Cloud Run Admin - run (cloudrun)
...
```

By adding `--all --long`, you can find complete list of available services with the maximum information. Also, [src/supported_api.rs](src/supported_api.rs) has lists of supported APIs.


### <a name='Listresourcesofaservice'></a>List resources of a service

Next, `zg ls <service>` shows the hierarchy of resources of a service. Most Google Cloud REST APIs define resources within such hierarchy, so showing them in a tree view would give you a comprehensive overview of the service.

```
$ zg ls gke
projects
  aggregated
    usableSubnetworks
  locations
    clusters
      nodePools
      well-known
    operations
  zones
    clusters
      nodePools
    operations
```

`gke` is an alias of `container` service, or [Google Kubernetes Engine (GKE)](https://cloud.google.com/kubernetes-engine). The output indicates that GKE's resource hierarchy starts with the top-level resource `projects`, followed by child resources like `locations`, and then `clusters` and `nodePools` under them.

You may notice that some resources have the same name but belong to different parent resources (e.g., `clusters`). See [Identify a resource uniquely](#Identifyaresourceuniquely) for more details.


### <a name='Listmethodsofaresource'></a>List methods of a resource

Executing `zg ls <service> <resource>` lists available methods associated with the resource.

```
$ zg ls bq tables
list
insert
delete
get
patch
update
getIamPolicy
setIamPolicy
testIamPermissions
```

Note that `bq` is an alias of `bigquery` service ([BigQuery](https://cloud.google.com/bigquery)). Options like `--long (-l)`, `--color (-C)` would give you more detailed information.

```
$ zg ls -lC bq tables
 method_name         http_method  path
 list                GET          projects/{projectsId}/datasets/{datasetsId}/tables
 insert              POST         projects/{projectsId}/datasets/{datasetsId}/tables
 delete              DELETE       projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}
 get                 GET          projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}
 patch               PATCH        projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}
 update              PUT          projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}
 getIamPolicy        POST         projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}:getIamPolicy
 setIamPolicy        POST         projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}:setIamPolicy
 testIamPermissions  POST         projects/{projectsId}/datasets/{datasetsId}/tables/{tablesId}:testIamPermissions
```

#### <a name='Identifyaresourceuniquely'></a>Identify a resource uniquely

As [mentioned earlier](#Listresourcesofaservice), different resources may have the same name. Different resources have different methods, URL to request, parameters it accepts. When possible, zygen automatically pick the most preferred one, but you may want to explicitly select the specific resource of your interest.

Here's the concept "resouce path" comes in. A resource path is a string represented by `<service_name>.<top_level_resource>. ... .<target_resource>` to uniquely identify a resource. You can find the full resource paths with `--long (-l)` option:

```
$ zg ls gke -l
resource_name      depth  resource_path                                     methods
projects           0      container.projects                                0
aggregated         1      container.projects.aggregated                     0
usableSubnetworks  2      container.projects.aggregated.usableSubnetworks   1    list
locations          1      container.projects.locations                      1    getServerConfig
clusters           2      container.projects.locations.clusters             19   get, list, create, delete, update, ...
nodePools          3      container.projects.locations.clusters.nodePools   10   get, list, create, delete, update, ...
well-known         3      container.projects.locations.clusters.well-known  1    getOpenid-configuration
operations         2      container.projects.locations.operations           3    get, list, cancel
zones              1      container.projects.zones                          1    getServerconfig
clusters           2      container.projects.zones.clusters                 17   get, list, addons, create, delete, ...
nodePools          3      container.projects.zones.clusters.nodePools       9    get, list, create, delete, update, ...
operations         2      container.projects.zones.operations               3    get, list, cancel
```

By passing a full or a partial resource path to uniquely identify a resource. The resource path assumption works with "ends_with" match, so `locations.clusters` (or even `ions.clusters` if you like) would enough to uniquely select `container.projects.locations.clusters` over `container.projects.zones.clusters`.

```
$ zg ls gke locations.clusters -l | grep logging
 setLogging                   POST         v1/projects/{projectsId}/locations/{locationsId}/clusters/{clustersId}:setLogging
$ zg ls gke zones.clusters -l | grep logging
 logging               POST         v1/projects/{projectId}/zones/{zone}/clusters/{clusterId}/logging
```


## <a name='zgdesc'></a>zg desc

The `zg desc (describe)` command shows detailed information of a method, which is helpful to execute the api (see `zg ex` below). For example, the following command shows detail of Cloud Composer's [create method](https://cloud.google.com/composer/docs/reference/rest/v1beta1/projects.locations.environments/create) for [the environments resource](https://cloud.google.com/composer/docs/reference/rest/v1beta1/projects.locations.environments#Environment):

```
$ zg desc composer environments create

method_name: create
method_id: composer.projects.locations.environments.create
http_method: POST
request_url: https://composer.googleapis.com/v1beta1/projects/{projectsId}/locations/{locationsId}/environments
autofill_params: projectsId, locationsId

required_params: None

minimum_data:
--data '{
  "name": ""
}'

Find API Reference: https://cloud.google.com/s/results/composer/docs?q=%22Method%3A%22%20projects.locations.environments%20create
```


## <a name='zgexec'></a>zg exec

`zg exec (ex)` command executes an API call. Parameters can be path parameters or query parameters, but you can use `--params (-p)` option to provide them. As the response is JSON, you can tune them using tools like `jq`.

```
$ zg ex spanner databases list -p instancesId=myins2 | \
    jq '.databases[] | .name'

"projects/my-project-12345/instances/myins2/databases/db1"
"projects/my-project-12345/instances/myins2/databases/testdb"
```

For POST/PUT/PATCH methods, you provide `--data (-d)` in JSON format.

```
$ zg ex composer environments create \
  --data '{
    "name": "projects/my-project-12345/locations/us-central1/environments/myzgenv"
  }'

{
  "metadata": {
    "@type": "type.googleapis.com/google.cloud.orchestration.airflow.service.v1beta1.OperationMetadata",
    "createTime": "2024-11-03T15:20:54.118810Z",
    "operationType": "CREATE",
    "resource": "projects/my-project-12345/locations/us-central1/environments/myzgenv",
    "resourceUuid": "xxxxxxxx-c663-4b40-bab0-xxxxxxxxxxxx",
    "state": "PENDING"
  },
  "name": "projects/my-project-12345/locations/us-central1/operations/xxxxxxxx-fe3a-4893-9ed1-xxxxxxxxxxxx"
}
```


### <a name='Equivalentcurl'></a>Equivalent curl

The `--equivalent-curl` option prints equivalent curl command.

```
$ zg ex spanner databases list -p instancesId=testins --equivalent-curl

curl -X GET \
  -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  -H "Content-Type: application/json; charset=utf-8" \
  "https://spanner.googleapis.com/v1/projects/my-project-12345/instances/testins/databases"
```

With this option, zygen works just as a command generator. It be useful to interact with APIs without zygen (e.g., when discussing with someone who don't use zygen).


## <a name='zgupdate'></a>zg update

We would recommend you to download the API definitions locally and convert them into zygen's internal format for best performance before using zygen. `zg update` command downloads the latest API definitions from [Google APIs Discovery Service](https://developers.google.com/discovery/v1/getting_started), and store them under `~/.config/zg/` directory. Zygen's API format is stored as the [MessagePack](https://msgpack.org/index.html) format.

```
$ zg update

$ tree -L 2 ~/.config/zg/
/Users/xxxxx/.config/zg/
├── api
│   ├── accessapproval_v1.msgpack
│   ├── accesscontextmanager_v1.msgpack
│   ├── advisorynotifications_v1.msgpack
│   ├── aiplatform_v1.msgpack
...
│   ├── workstations_v1.msgpack
│   └── workstations_v1beta.msgpack
└── discovered
    ├── _discovered_apis.json
    ├── accessapproval_v1.json
    ├── accesscontextmanager_v1.json
    ├── advisorynotifications_v1.json
    ├── aiplatform_v1.json
    ...
    ├── workstations_v1.json
    └── workstations_v1beta.json
```

Note that `zg update` is not mandatory; you can rely on the lazy loading mechanism of zygen, which automatically downloads the API definitions when needed.


# <a name='Installation'></a>Installation

Dependencies:
- [gcloud](https://cloud.google.com/sdk/docs/install-sdk)
  - zygen relies on `gcloud` for [generating access token](https://cloud.google.com/sdk/gcloud/reference/auth/print-access-token) and [retrieving project id from its config](https://cloud.google.com/sdk/gcloud/reference/config/get).
  - Make sure you have installed and initialize the `gcloud` command in your `$PATH`.


## <a name='HomebrewMacOSLinux'></a>Homebrew (MacOS/Linux)

```
$ brew install thash/tap/zygen
```


## <a name='Downloadbinary'></a>Download binary

Find [the latest release](https://github.com/thash/zygen/releases/latest) and download the binary for your platform. Extract the archive and make the binary executable:

```bash
$ tar xvzf zygen-x.x.x-your-platform.tar.gz
$ chmod +x zg
$ ./zg
```

Then you can move the `zg` binary to anywhere in your `$PATH`.

Note that your system may block the binary from running due to their security policies. For example, [MacOS blocks applications from unknown locations](https://support.apple.com/en-us/102445), so you may need to explicitly allow it.


## <a name='Installfromsource'></a>Install from source

Assuming you have [Rust](https://www.rust-lang.org/tools/install) installed, you can build zygen from source:

```bash
$ git clone https://github.com/thash/zygen; cd zygen
$ cargo build --release
$ ./target/release/zg
```


# Examples

Random examples to show how zygen helps you to explorer Google Cloud APIs.

```
$ zg ls bigquery
projects
  datasets
    models
    routines
    tables
      tabledata
      rowAccessPolicies
  jobs

$ zg ex resource-manager projects getIamPolicy \
  | jq '.bindings[] | .members[]' | sort | uniq
"serviceAccount:0000000000000-compute@developer.gserviceaccount.com"
"serviceAccount:service-0000000000000@cloudcomposer-accounts.iam.gserviceaccount.com"
"serviceAccount:service-0000000000000@container-engine-robot.iam.gserviceaccount.com"
"serviceAccount:service-0000000000000@containerregistry.iam.gserviceaccount.com"
"serviceAccount:service-0000000000000@gcp-sa-cloudbuild.iam.gserviceaccount.com"
"serviceAccount:service-0000000000000@serverless-robot-prod.iam.gserviceaccount.com"
...
"user:xxxxxx@example.com"
```


# <a name='Misc'></a>Misc

## <a name='Whatzygenisnotfor'></a>What zygen is not for

zygen is not a Google official product, and not intended for use in a production environment. For a production environment, we recommend to use official [Cloud Client Libraries](https://cloud.google.com/apis/docs/cloud-client-libraries) that offer more optimized developer experience and robust API interactions. For user-friendliness with advanced features and abstractions, please explore [gcloud](https://cloud.google.com/sdk/gcloud) or service-specific options like [bq](https://cloud.google.com/bigquery/docs/bq-command-line-tool).

The web-based [Google APIs Explorer](https://developers.google.com/apis-explorer) enable you to explore the APIs and execute them. zygen is designed for developers who prefer CLI over GUI, and who want to interact with APIs in faster, [sharable](#equivalent-curl), and scriptable ways.

