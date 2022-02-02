.PHONY: all create-repository update-repository build-image deploy-image create-resources update-resources deploy-changes

REPO_STACK_NAME = PeridotGithubActivityNotificationRepository
REPO_NAME = peridot/github-activity-notification
APP_STACK_NAME = PeridotGithubActivityNotificationApp
REMOTE_MASTER_REPO_URL = $(AWS_CLIENT_ID).dkr.ecr.$(AWS_REGION).amazonaws.com/$(REPO_NAME):master
ENABLE_APP_DEBUG_LOG ?= false

all: deploy-changes

deploy-changes: update-repository deploy-image update-resources

create-repository:
	aws cloudformation create-stack \
	--stack-name $(REPO_STACK_NAME) \
	--template-body file://$(CURDIR)/repository.cf.yml \
	--parameters ParameterKey=RepositoryName,ParameterValue=$(REPO_NAME)

update-repository: repository.cf.yml
	aws cloudformation deploy --stack-name $(REPO_STACK_NAME) --template-file $(CURDIR)/repository.cf.yml --no-fail-on-empty-changeset

build-image: Dockerfile .dockerignore Cargo.toml Cargo.lock src/*
	docker build -t $(REPO_NAME) .

deploy-image: build-image
	docker tag $(REPO_NAME):latest $(REMOTE_MASTER_REPO_URL)
	docker push $(REMOTE_MASTER_REPO_URL)

refresh-function-code:
	aws lambda update-function-code --function-name Peridot-GithubActivityNotification --image-uri $(REMOTE_MASTER_REPO_URL)

create-resources:
	aws cloudformation create-stack \
	--stack-name $(APP_STACK_NAME) \
	--template-body file://$(CURDIR)/resources.cf.yml \
	--capabilities CAPABILITY_IAM \
	--parameters ParameterKey=RepositoryStackName,ParameterValue=$(REPO_STACK_NAME) \
	ParameterKey=TargetPath,ParameterValue=/peridot ParameterKey=EnableDebugLog,ParameterValue=$(ENABLE_APP_DEBUG_LOG)

update-resources: resources.cf.yml
	aws cloudformation deploy --stack-name $(APP_STACK_NAME) --template-file $(CURDIR)/resources.cf.yml --capabilities CAPABILITY_IAM --no-fail-on-empty-changeset \
	--parameter-overrides EnableDebugLog=$(ENABLE_APP_DEBUG_LOG)
